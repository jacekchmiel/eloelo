//! Bridges the `MessageBus` of eloelo with the `MessageBus` of a remote Discord bot server
//! (elodisco binary) over a WebSocket. eloelo is the client, the bot server listens.
//!
//! Messages are forwarded as JSON, unchanged. Which messages cross the bridge is decided only by
//! `to_bot` and `to_eloelo` filters below. The filters must not overlap, otherwise a message would
//! be bounced back and forth between the buses.

use std::time::{Duration, Instant};

use anyhow::{bail, Context as _, Result};
use futures_util::{SinkExt as _, StreamExt as _};
use log::{debug, info, warn};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{header, HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::message_bus::{Event, FinishMatch, Message, MessageBus, UiCommand, UiUpdate};
use crate::utils::print_err;

const PING_INTERVAL: Duration = Duration::from_secs(30);
const PEER_TIMEOUT: Duration = Duration::from_secs(90);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

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
pub async fn run_client(url: String, secret: String, message_bus: MessageBus) {
    if secret.is_empty() {
        warn!("Bridge: bridgeSecret is not set, not connecting to the Discord bot.");
        return;
    }
    loop {
        match connect(&url, &secret).await {
            Ok(ws) => {
                info!("Bridge: connected to {url}");
                match bridge(ws, &message_bus, to_bot, to_eloelo).await {
                    Ok(()) => info!("Bridge: connection closed."),
                    Err(e) => warn!("Bridge: connection lost: {e:#}"),
                }
            }
            Err(e) => warn!("Bridge: failed to connect to {url}: {e:#}"),
        }
        tokio::time::sleep(RECONNECT_DELAY).await;
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
            match bridge(ws, &message_bus, to_eloelo, to_bot).await {
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
    let (ws, _) = tokio_tungstenite::connect_async(request).await?;
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
    outgoing: fn(&Message) -> bool,
    incoming: fn(&Message) -> bool,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut sink, mut stream) = ws.split();
    let mut local_messages = message_bus.subscribe().stream().boxed();
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

    #[tokio::test]
    async fn forwards_only_bridged_messages() {
        let bot_bus = MessageBus::new();
        let eloelo_bus = MessageBus::new();
        let mut bot_messages = bot_bus.subscribe().stream().boxed();
        let addr = start_server(bot_bus.clone()).await;
        tokio::spawn(run_client(
            format!("ws://{addr}"),
            SECRET.into(),
            eloelo_bus.clone(),
        ));

        // Messages sent before the client subscribes are lost, so keep sending until one arrives.
        loop {
            eloelo_bus.send(Message::UiCommand(UiCommand::InitializeUi));
            let received = tokio::time::timeout(Duration::from_millis(50), bot_messages.next());
            if received.await.is_ok() {
                break;
            }
        }

        eloelo_bus.send(Message::UiCommand(UiCommand::StartMatch));
        eloelo_bus.send(Message::UiCommand(UiCommand::FinishMatch(
            FinishMatch::Cancelled,
        )));
        let received = loop {
            match next_message(&mut bot_messages).await {
                Message::UiCommand(UiCommand::InitializeUi) => continue,
                message => break message,
            }
        };
        assert!(matches!(
            received,
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
