//! Bridges the `MessageBus` of eloelo with the `MessageBus` of a remote Discord bot server
//! (elodisco binary) over a WebSocket. eloelo is the client, the bot server listens.
//!
//! Messages are forwarded as JSON, unchanged. Which messages cross the bridge is decided only by
//! `to_bot` and `to_eloelo` filters below. The filters must not overlap, otherwise a message would
//! be bounced back and forth between the buses.

use std::time::{Duration, Instant};

use anyhow::{bail, Context as _, Result};
use chrono::Local;
use futures_util::{SinkExt as _, StreamExt as _};
use log::{debug, info, warn};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{header, HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::message_bus::{
    DiscordBotStatus, Event, FinishMatch, Message, MessageBus, MessageBusSubscription, UiCommand,
    UiUpdate,
};
use crate::utils::print_err;

const PING_INTERVAL: Duration = Duration::from_secs(5);
const PEER_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// Messages forwarded from eloelo to the Discord bot.
fn to_bot(message: &Message) -> bool {
    matches!(
        message,
        Message::Event(Event::MatchStart(_) | Event::RichMatchResult(_))
            | Message::UiCommand(
                UiCommand::InitializeUi | UiCommand::FinishMatch(FinishMatch::Cancelled)
            )
    )
}

/// Messages forwarded from the Discord bot to eloelo.
fn to_eloelo(message: &Message) -> bool {
    matches!(
        message,
        Message::Event(Event::HeroesAssigned(_)) | Message::UiUpdate(UiUpdate::DiscordInfo(_))
    )
}

/// eloelo side. Connects to the bot server and keeps reconnecting when the connection is lost.
/// Publishes `DiscordBotStatus` changes on the local bus.
pub async fn run_client(url: String, secret: String, message_bus: MessageBus) {
    let mut status = StatusReporter::new(message_bus.clone());
    if secret.is_empty() {
        warn!("Bridge: bridgeSecret is not set, not connecting to the Discord bot.");
        status.disconnected("bridgeSecret is not set".into());
        return;
    }
    loop {
        let error = match connect(&url, &secret).await {
            Ok(ws) => {
                info!("Bridge: connected to {url}");
                // Subscribe before reporting, so no message sent after the status change is missed.
                let local_messages = message_bus.subscribe();
                status.connected();
                match bridge(ws, &message_bus, local_messages, to_bot, to_eloelo).await {
                    Ok(()) => "Connection closed".into(),
                    Err(e) => format!("Connection lost: {e:#}"),
                }
            }
            Err(e) => format!("Failed to connect: {e:#}"),
        };
        if status.disconnected(error.clone()) {
            warn!("Bridge: {url}: {error}");
        }
        tokio::time::sleep(RECONNECT_DELAY).await;
    }
}

/// Publishes bridge status on the local bus, only when it changes.
struct StatusReporter {
    message_bus: MessageBus,
    last: Option<DiscordBotStatus>,
}

impl StatusReporter {
    fn new(message_bus: MessageBus) -> Self {
        Self {
            message_bus,
            last: None,
        }
    }

    fn connected(&mut self) {
        self.report(DiscordBotStatus::Connected);
    }

    fn disconnected(&mut self, error: String) -> bool {
        let since = match &self.last {
            Some(DiscordBotStatus::Disconnected { since, .. }) => *since,
            _ => Local::now(),
        };
        self.report(DiscordBotStatus::Disconnected { since, error })
    }

    fn report(&mut self, status: DiscordBotStatus) -> bool {
        if self.last.as_ref() == Some(&status) {
            return false;
        }
        self.message_bus
            .send(Message::Event(Event::DiscordBotStatus(status.clone())));
        self.last = Some(status);
        true
    }
}

/// Bot server side. Accepts eloelo connections.
pub async fn run_server(addr: &str, secret: &str, message_bus: MessageBus) -> Result<()> {
    if secret.is_empty() {
        bail!("bridgeSecret is not set.");
    }
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind {addr}"))?;
    info!("Bridge: listening on {addr}");
    loop {
        let (tcp, peer) = listener.accept().await.context("accept")?;
        let secret = secret.to_owned();
        let message_bus = message_bus.clone();
        tokio::spawn(async move {
            let authorize = |request: &Request, response| authorize(request, response, &secret);
            let ws = match tokio_tungstenite::accept_hdr_async(tcp, authorize).await {
                Ok(ws) => ws,
                Err(e) => {
                    warn!("Bridge: rejected connection from {peer}: {e}");
                    return;
                }
            };
            info!("Bridge: eloelo connected from {peer}");
            let local_messages = message_bus.subscribe();
            match bridge(ws, &message_bus, local_messages, to_eloelo, to_bot).await {
                Ok(()) => info!("Bridge: {peer} disconnected."),
                Err(e) => warn!("Bridge: connection with {peer} lost: {e:#}"),
            }
        });
    }
}

async fn connect(url: &str, secret: &str) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let mut request = url.into_client_request()?;
    request.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {secret}"))?,
    );
    let (ws, _) = tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(request))
        .await
        .context("Timed out")??;
    Ok(ws)
}

fn authorize(
    request: &Request,
    response: Response,
    secret: &str,
) -> Result<Response, ErrorResponse> {
    let expected = format!("Bearer {secret}");
    match request.headers().get(header::AUTHORIZATION) {
        Some(value) if value == expected.as_str() => Ok(response),
        _ => {
            let mut error = ErrorResponse::new(Some("Unauthorized".into()));
            *error.status_mut() = StatusCode::UNAUTHORIZED;
            Err(error)
        }
    }
}

/// Forwards `outgoing` messages from the local bus to the socket and `incoming` messages from the
/// socket to the local bus. Returns when the connection is closed.
async fn bridge<S>(
    ws: WebSocketStream<S>,
    message_bus: &MessageBus,
    local_messages: MessageBusSubscription,
    outgoing: fn(&Message) -> bool,
    incoming: fn(&Message) -> bool,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut sink, mut stream) = ws.split();
    let mut local_messages = local_messages.stream().boxed();
    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_seen = Instant::now();
    loop {
        tokio::select! {
            ws_message = stream.next() => {
                last_seen = Instant::now();
                match ws_message.transpose()? {
                    Some(WsMessage::Text(text)) => match serde_json::from_str(text.as_str()) {
                        Ok(message) if incoming(&message) => {
                            debug!("Bridge: received {message:?}");
                            message_bus.send(message);
                        }
                        Ok(message) => warn!("Bridge: unexpected message dropped: {message:?}"),
                        Err(e) => warn!("Bridge: failed to parse message: {e}"),
                    },
                    Some(WsMessage::Close(_)) | None => return Ok(()),
                    Some(_) => {}
                }
            }
            message = local_messages.next() => match message {
                Some(Ok(message)) if outgoing(&message) => {
                    debug!("Bridge: sending {message:?}");
                    sink.send(WsMessage::text(serde_json::to_string(&message)?)).await?;
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => print_err(&e),
                None => return Ok(()),
            },
            _ = ping.tick() => {
                if last_seen.elapsed() > PEER_TIMEOUT {
                    bail!("No response from peer for {PEER_TIMEOUT:?}");
                }
                sink.send(WsMessage::Ping(Default::default())).await?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elodisco::dota_bot::Hero;
    use crate::elodisco::hero_assignment_strategy::HeroTag;
    use crate::message_bus::{HeroesAssigned, MatchStart, MatchStartTeam};
    use eloelo_model::player::{Player, PlayerConfig, PlayerDb};
    use futures_util::Stream;
    use std::collections::HashMap;

    const SECRET: &str = "secret";

    async fn start_server(message_bus: MessageBus) -> String {
        let addr = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();
        tokio::spawn({
            let addr = addr.clone();
            async move { run_server(&addr, SECRET, message_bus).await }
        });
        while TcpStream::connect(&addr).await.is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        addr
    }

    async fn next_message(messages: &mut (impl Stream<Item = Result<Message>> + Unpin)) -> Message {
        tokio::time::timeout(Duration::from_secs(1), messages.next())
            .await
            .expect("timeout")
            .expect("stream finished")
            .expect("message")
    }

    #[tokio::test]
    async fn rejects_wrong_secret() {
        let addr = start_server(MessageBus::new()).await;
        let url = format!("ws://{addr}");
        assert!(connect(&url, "wrong").await.is_err());
        assert!(connect(&url, SECRET).await.is_ok());
    }

    async fn next_status(
        messages: &mut (impl Stream<Item = Result<Message>> + Unpin),
    ) -> DiscordBotStatus {
        loop {
            if let Message::Event(Event::DiscordBotStatus(status)) = next_message(messages).await {
                return status;
            }
        }
    }

    #[tokio::test]
    async fn forwards_only_bridged_messages() {
        let bot_bus = MessageBus::new();
        let eloelo_bus = MessageBus::new();
        let mut bot_messages = bot_bus.subscribe().stream().boxed();
        let mut eloelo_messages = eloelo_bus.subscribe().stream().boxed();
        let addr = start_server(bot_bus.clone()).await;
        tokio::spawn(run_client(
            format!("ws://{addr}"),
            SECRET.into(),
            eloelo_bus.clone(),
        ));
        assert_eq!(
            next_status(&mut eloelo_messages).await,
            DiscordBotStatus::Connected
        );

        eloelo_bus.send(Message::UiCommand(UiCommand::StartMatch));
        eloelo_bus.send(Message::UiCommand(UiCommand::FinishMatch(
            FinishMatch::Cancelled,
        )));
        assert!(matches!(
            next_message(&mut bot_messages).await,
            Message::UiCommand(UiCommand::FinishMatch(FinishMatch::Cancelled))
        ));

        let mut eloelo_messages = eloelo_bus.subscribe().stream().boxed();
        bot_bus.send(Message::UiCommand(UiCommand::ShuffleTeams));
        bot_bus.send(Message::UiUpdate(UiUpdate::DiscordInfo(vec![])));
        assert!(matches!(
            next_message(&mut eloelo_messages).await,
            Message::UiUpdate(UiUpdate::DiscordInfo(_))
        ));
    }

    #[tokio::test]
    async fn reports_connection_status() {
        // Server accepting a single connection and dropping it right away.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            drop(tokio_tungstenite::accept_async(tcp).await.unwrap());
        });
        let eloelo_bus = MessageBus::new();
        let mut eloelo_messages = eloelo_bus.subscribe().stream().boxed();
        tokio::spawn(run_client(
            format!("ws://{addr}"),
            SECRET.into(),
            eloelo_bus.clone(),
        ));

        assert_eq!(
            next_status(&mut eloelo_messages).await,
            DiscordBotStatus::Connected
        );
        assert!(matches!(
            next_status(&mut eloelo_messages).await,
            DiscordBotStatus::Disconnected { .. }
        ));
    }

    #[test]
    fn bridged_messages_survive_json() {
        let player = Player::from(PlayerConfig {
            id: "p1".into(),
            discord_username: Some("user".into()),
            ..Default::default()
        });
        let team = |name: &str| MatchStartTeam {
            players: HashMap::from([("p1".into(), 1000)]),
            name: name.into(),
        };
        let match_start = MatchStart {
            game: "Dota".into(),
            player_db: PlayerDb::new([player]),
            left_team: team("Radiant"),
            right_team: team("Dire"),
        };
        let heroes_assigned = HeroesAssigned {
            match_start: match_start.clone(),
            assignments: HashMap::from([("user".into(), vec![Hero::from("Abaddon")])]),
            tags: HashMap::from([("user".into(), HeroTag::Support)]),
        };
        for message in [
            Message::Event(Event::MatchStart(match_start)),
            Message::Event(Event::HeroesAssigned(heroes_assigned)),
        ] {
            let json = serde_json::to_value(&message).unwrap();
            let parsed: Message = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(json, serde_json::to_value(&parsed).unwrap());
        }
    }
}
