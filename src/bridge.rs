//! Bridges the `MessageBus` of eloelo with the `MessageBus` of a remote Discord bot server
//! (elodisco binary) over a WebSocket. eloelo is the client, the bot server listens.
//!
//! Messages are forwarded as JSON, unchanged. Which messages cross the bridge is decided only by
//! `to_bot` and `to_eloelo` filters below. The filters must not overlap, otherwise a message would
//! be bounced back and forth between the buses.
//!
//! The bot server serves one eloelo at a time. The bot keeps per-match state and its replies go to
//! every connected client, so a second eloelo would mix its matches with the first one's.
//!
//! eloelo sends its test mode in the handshake, so the bot follows it without its own config.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context as _, Result};
use chrono::Local;
use futures_util::{SinkExt as _, StreamExt as _};
use log::{debug, info, warn};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request};
use tokio_tungstenite::tungstenite::http::{header, HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::{Error as WsError, Message as WsMessage};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_util::sync::CancellationToken;

use crate::message_bus::{
    DiscordBotStatus, Event, FinishMatch, Message, MessageBus, MessageBusSubscription, UiCommand,
    UiUpdate,
};
use crate::utils::print_err;

const PING_INTERVAL: Duration = Duration::from_secs(5);
const PEER_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);
const TEST_MODE_HEADER: &str = "x-eloelo-test-mode";

/// Messages forwarded from eloelo to the Discord bot.
fn to_bot(message: &Message) -> bool {
    matches!(
        message,
        Message::Event(
            Event::MatchStart(_) | Event::RichMatchResult(_) | Event::PlayerDbUpdated(_)
        ) | Message::UiCommand(
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

/// eloelo side. Connects to the bot server and keeps reconnecting when the connection is lost,
/// until `shutdown` is cancelled. Publishes `DiscordBotStatus` changes on the local bus.
pub async fn run_client(
    url: String,
    secret: String,
    test_mode: bool,
    message_bus: MessageBus,
    shutdown: CancellationToken,
) {
    info!("Bridge: sending test mode: {test_mode}");
    let mut status = StatusReporter::new(message_bus.clone());
    if secret.is_empty() {
        status.disconnected("bridgeSecret is not set, not connecting".into());
        return;
    }
    loop {
        let connection = tokio::select! {
            connection = connect(&url, &secret, test_mode) => connection,
            _ = shutdown.cancelled() => return,
        };
        match connection {
            Ok(ws) => {
                // Subscribe before reporting, so no message sent after the status change is missed.
                let local_messages = message_bus.subscribe();
                status.connected();
                let result = bridge(
                    ws,
                    &message_bus,
                    local_messages,
                    to_bot,
                    to_eloelo,
                    &shutdown,
                )
                .await;
                if shutdown.is_cancelled() {
                    info!("Bridge: connection closed.");
                    return;
                }
                status.disconnected(match result {
                    Ok(()) => "Bot server closed the connection".into(),
                    Err(e) => format!("Connection lost: {}", error_chain(&e)),
                });
            }
            Err(e) => {
                status.disconnected(format!("Failed to connect to {url}: {}", error_chain(&e)));
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(RECONNECT_DELAY) => {}
            _ = shutdown.cancelled() => return,
        }
    }
}

/// Publishes bridge status on the local bus and logs it, only when it changes. Retries failing
/// the same way don't repeat the log.
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

    fn disconnected(&mut self, error: String) {
        let since = match &self.last {
            Some(DiscordBotStatus::Disconnected { since, .. }) => *since,
            _ => Local::now(),
        };
        self.report(DiscordBotStatus::Disconnected { since, error });
    }

    fn report(&mut self, status: DiscordBotStatus) {
        if self.last.as_ref() == Some(&status) {
            return;
        }
        match &status {
            DiscordBotStatus::Disconnected { error, .. } => warn!("Bridge: {error}"),
            _ => info!("Bridge: connected."),
        }
        self.message_bus
            .send(Message::Event(Event::DiscordBotStatus(status.clone())));
        self.last = Some(status);
    }
}

/// Bot server side. Accepts eloelo connections until `shutdown` is cancelled, then closes them.
/// While one eloelo is connected, others are rejected with 409 Conflict.
pub async fn run_server(
    addr: &str,
    secret: &str,
    message_bus: MessageBus,
    shutdown: CancellationToken,
) -> Result<()> {
    if secret.is_empty() {
        bail!("bridgeSecret is not set.");
    }
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind {addr}"))?;
    info!("Bridge: listening on {addr}");
    let slot = Arc::new(Semaphore::new(1));
    let mut connections = JoinSet::new();
    loop {
        let (tcp, peer) = tokio::select! {
            accepted = listener.accept() => accepted.context("accept")?,
            _ = shutdown.cancelled() => break,
        };
        while connections.try_join_next().is_some() {}
        connections.spawn(serve(
            tcp,
            peer,
            secret.to_owned(),
            slot.clone(),
            message_bus.clone(),
            shutdown.clone(),
        ));
    }
    connections.join_all().await;
    Ok(())
}

/// Bot server side of a single connection, from the handshake until it closes.
async fn serve(
    tcp: TcpStream,
    peer: SocketAddr,
    secret: String,
    slot: Arc<Semaphore>,
    message_bus: MessageBus,
    shutdown: CancellationToken,
) {
    let mut permit = None;
    let mut test_mode = None;
    let handshake = |request: &Request, response| {
        permit = Some(admit(request, &secret, slot)?);
        test_mode = request
            .headers()
            .get(TEST_MODE_HEADER)
            .map(|value| value == "true");
        Ok(response)
    };
    let ws = match tokio_tungstenite::accept_hdr_async(tcp, handshake).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!("Bridge: rejected connection from {peer}: {e}");
            return;
        }
    };
    let test_mode_log = test_mode.map_or("not sent".into(), |t| t.to_string());
    info!("Bridge: eloelo connected from {peer}, test mode: {test_mode_log}");
    // Sent before any bridged message, so the bot is in the right mode for the first match. A
    // client without the header leaves the bot in its current mode.
    if let Some(test_mode) = test_mode {
        message_bus.send(Message::Event(Event::TestMode(test_mode)));
    }
    let local_messages = message_bus.subscribe();
    match bridge(
        ws,
        &message_bus,
        local_messages,
        to_eloelo,
        to_bot,
        &shutdown,
    )
    .await
    {
        Ok(()) => info!("Bridge: connection with {peer} closed."),
        Err(e) => warn!("Bridge: connection with {peer} lost: {}", error_chain(&e)),
    }
    // Frees the slot for the next eloelo.
    drop(permit);
}

/// Lets a connecting eloelo in if it knows the secret and no other eloelo is connected. The
/// secret is checked first, so strangers can't learn whether the slot is taken.
fn admit(
    request: &Request,
    secret: &str,
    slot: Arc<Semaphore>,
) -> Result<OwnedSemaphorePermit, ErrorResponse> {
    let expected = format!("Bearer {secret}");
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .is_some_and(|value| value == expected.as_str());
    if !authorized {
        return Err(error_response(StatusCode::UNAUTHORIZED, "Unauthorized"));
    }
    slot.try_acquire_owned()
        .map_err(|_| error_response(StatusCode::CONFLICT, "Another eloelo is connected"))
}

async fn connect(
    url: &str,
    secret: &str,
    test_mode: bool,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let mut request = url.into_client_request()?;
    let headers = request.headers_mut();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {secret}"))?,
    );
    headers.insert(
        TEST_MODE_HEADER,
        HeaderValue::from_static(if test_mode { "true" } else { "false" }),
    );
    let connection =
        tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(request))
            .await
            .context("Timed out")?;
    match connection {
        Ok((ws, _)) => Ok(ws),
        Err(WsError::Http(response)) if response.status() == StatusCode::CONFLICT => {
            bail!("Another eloelo is already connected to the bot server")
        }
        Err(e) => Err(e.into()),
    }
}

fn error_response(status: StatusCode, body: &str) -> ErrorResponse {
    let mut error = ErrorResponse::new(Some(body.into()));
    *error.status_mut() = status;
    error
}

/// Like `{e:#}`, but skips causes already contained in the message above them. tungstenite errors
/// repeat their source in their own message.
fn error_chain(e: &anyhow::Error) -> String {
    let mut out = String::new();
    for cause in e.chain().map(ToString::to_string) {
        if out.contains(&cause) {
            continue;
        }
        if !out.is_empty() {
            out.push_str(": ");
        }
        out.push_str(&cause);
    }
    out
}

/// Forwards `outgoing` messages from the local bus to the socket and `incoming` messages from the
/// socket to the local bus. Returns when the connection is closed by either side, or after
/// closing it when `shutdown` is cancelled.
async fn bridge<S>(
    ws: WebSocketStream<S>,
    message_bus: &MessageBus,
    local_messages: MessageBusSubscription,
    outgoing: fn(&Message) -> bool,
    incoming: fn(&Message) -> bool,
    shutdown: &CancellationToken,
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
                    Some(WsMessage::Close(_)) => {
                        // Confirm the peer's closing handshake.
                        let _ = sink.close().await;
                        return Ok(());
                    }
                    Some(_) => {}
                    None => return Ok(()),
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
            _ = shutdown.cancelled() => {
                // Closing handshake: send Close and give the peer a moment to confirm.
                sink.close().await?;
                let confirmed = async { while let Some(Ok(_)) = stream.next().await {} };
                let _ = tokio::time::timeout(CLOSE_TIMEOUT, confirmed).await;
                return Ok(());
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

    async fn start_server(
        message_bus: MessageBus,
        shutdown: CancellationToken,
    ) -> (String, tokio::task::JoinHandle<Result<()>>) {
        let addr = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string();
        let server = tokio::spawn({
            let addr = addr.clone();
            async move { run_server(&addr, SECRET, message_bus, shutdown).await }
        });
        while TcpStream::connect(&addr).await.is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        (addr, server)
    }

    /// Reads the socket until it ends. Returns whether Close was received and the error, if any.
    async fn read_until_end<S: AsyncRead + AsyncWrite + Unpin>(
        mut ws: WebSocketStream<S>,
    ) -> (bool, Option<String>) {
        let mut close_received = false;
        while let Some(message) = ws.next().await {
            match message {
                Ok(WsMessage::Close(_)) => close_received = true,
                Ok(_) => {}
                Err(e) => return (close_received, Some(e.to_string())),
            }
        }
        (close_received, None)
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
        let (addr, _) = start_server(MessageBus::new(), CancellationToken::new()).await;
        let url = format!("ws://{addr}");
        assert!(connect(&url, "wrong", false).await.is_err());
        assert!(connect(&url, SECRET, false).await.is_ok());
    }

    #[tokio::test]
    async fn serves_one_client_at_a_time() {
        let (addr, _) = start_server(MessageBus::new(), CancellationToken::new()).await;
        let url = format!("ws://{addr}");
        let mut first = connect(&url, SECRET, false).await.unwrap();
        let e = connect(&url, SECRET, false).await.unwrap_err();
        assert!(e.to_string().contains("Another eloelo"), "{e:#}");

        first.close(None).await.unwrap();
        let reconnect = async {
            while connect(&url, SECRET, false).await.is_err() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(2), reconnect)
            .await
            .expect("slot not released");
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
    async fn forwards_test_mode_and_only_bridged_messages() {
        let bot_bus = MessageBus::new();
        let eloelo_bus = MessageBus::new();
        let mut bot_messages = bot_bus.subscribe().stream().boxed();
        let mut eloelo_messages = eloelo_bus.subscribe().stream().boxed();
        let (addr, _) = start_server(bot_bus.clone(), CancellationToken::new()).await;
        tokio::spawn(run_client(
            format!("ws://{addr}"),
            SECRET.into(),
            true,
            eloelo_bus.clone(),
            CancellationToken::new(),
        ));
        assert_eq!(
            next_status(&mut eloelo_messages).await,
            DiscordBotStatus::Connected
        );
        assert!(matches!(
            next_message(&mut bot_messages).await,
            Message::Event(Event::TestMode(true))
        ));

        eloelo_bus.send(Message::UiCommand(UiCommand::StartMatch));
        eloelo_bus.send(Message::UiCommand(UiCommand::FinishMatch(
            FinishMatch::Cancelled,
        )));
        assert!(matches!(
            next_message(&mut bot_messages).await,
            Message::UiCommand(UiCommand::FinishMatch(FinishMatch::Cancelled))
        ));
        eloelo_bus.send(Message::Event(Event::PlayerDbUpdated(PlayerDb::new([]))));
        assert!(matches!(
            next_message(&mut bot_messages).await,
            Message::Event(Event::PlayerDbUpdated(_))
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
            false,
            eloelo_bus.clone(),
            CancellationToken::new(),
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

    #[tokio::test]
    async fn client_closes_gracefully_on_shutdown() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            read_until_end(tokio_tungstenite::accept_async(tcp).await.unwrap()).await
        });
        let eloelo_bus = MessageBus::new();
        let mut eloelo_messages = eloelo_bus.subscribe().stream().boxed();
        let shutdown = CancellationToken::new();
        let client = tokio::spawn(run_client(
            format!("ws://{addr}"),
            SECRET.into(),
            false,
            eloelo_bus.clone(),
            shutdown.clone(),
        ));
        assert_eq!(
            next_status(&mut eloelo_messages).await,
            DiscordBotStatus::Connected
        );

        shutdown.cancel();
        let timeout = Duration::from_secs(2);
        tokio::time::timeout(timeout, client)
            .await
            .unwrap()
            .unwrap();
        let server = tokio::time::timeout(timeout, server).await.unwrap();
        assert_eq!(server.unwrap(), (true, None));
    }

    #[tokio::test]
    async fn server_closes_gracefully_on_shutdown() {
        let shutdown = CancellationToken::new();
        let (addr, server) = start_server(MessageBus::new(), shutdown.clone()).await;
        let ws = connect(&format!("ws://{addr}"), SECRET, false)
            .await
            .unwrap();

        shutdown.cancel();
        let timeout = Duration::from_secs(2);
        let client = tokio::time::timeout(timeout, read_until_end(ws)).await;
        assert_eq!(client.unwrap(), (true, None));
        let server = tokio::time::timeout(timeout, server).await.unwrap();
        server.unwrap().unwrap();
    }

    #[test]
    fn error_chain_skips_repeated_causes() {
        let e = anyhow::Error::from(std::io::Error::other("reset"))
            .context("protocol error: reset")
            .context("Connection lost");
        assert_eq!(error_chain(&e), "Connection lost: protocol error: reset");
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
        let player_db = PlayerDb::new([player]);
        let match_start = MatchStart {
            game: "Dota".into(),
            player_db: player_db.clone(),
            left_team: team("Radiant"),
            right_team: team("Dire"),
            hero_assignment: Default::default(),
        };
        let heroes_assigned = HeroesAssigned {
            match_start: match_start.clone(),
            assignments: HashMap::from([("user".into(), vec![Hero::from("Abaddon")])]),
            tags: HashMap::from([("user".into(), HeroTag::Support)]),
        };
        for message in [
            Message::Event(Event::MatchStart(match_start)),
            Message::Event(Event::HeroesAssigned(heroes_assigned)),
            Message::Event(Event::PlayerDbUpdated(player_db)),
        ] {
            let json = serde_json::to_value(&message).unwrap();
            let parsed: Message = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(json, serde_json::to_value(&parsed).unwrap());
        }
    }

    #[test]
    fn match_start_from_older_eloelo_uses_default_hero_assignment() {
        let json = serde_json::json!({"Event": {"MatchStart": {
            "game": "Dota",
            "player_db": PlayerDb::new([]),
            "left_team": {"players": {}, "name": "Radiant"},
            "right_team": {"players": {}, "name": "Dire"},
        }}});
        let Message::Event(Event::MatchStart(match_start)) = serde_json::from_value(json).unwrap()
        else {
            panic!("not a MatchStart");
        };
        assert_eq!(match_start.hero_assignment, Default::default());
    }
}
