use anyhow::Result;
use bytes::Bytes;
use eloelo_model::player::{DiscordUsername, Player, PlayerDb};
use eloelo_model::{GameId, PlayerId, Team, WinScale};
use futures_util::{Stream, StreamExt};
use log::error;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::wrappers::BroadcastStream;

use super::ui_state::UiState;

#[derive(Clone)]
pub(crate) struct MessageBus(Sender<Message>);

impl MessageBus {
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(100);
        Self(sender)
    }

    pub fn send(&self, message: Message) {
        if let Err(message) = self.0.send(message) {
            error!("Message not sent {:?}", message);
        }
    }

    pub fn subscribe(&self) -> MessageBusSubscription {
        MessageBusSubscription(self.0.subscribe())
    }
}

pub(crate) struct MessageBusSubscription(Receiver<Message>);

impl MessageBusSubscription {
    pub async fn recv(&mut self) -> Option<Message> {
        Self::translate_recv(self.0.recv().await)
    }

    pub fn stream(self) -> impl Stream<Item = Result<Message>> {
        BroadcastStream::new(self.0).map(|r| r.map_err(anyhow::Error::from))
    }

    pub fn ui_update_stream(self) -> impl Stream<Item = Result<UiUpdate>> {
        self.stream().filter_map(|r| async move {
            match r {
                Ok(Message::UiUpdate(ui_update)) => Some(Ok(ui_update)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        })
    }

    pub fn ui_command_stream(self) -> impl Stream<Item = Result<UiCommand>> {
        self.stream().filter_map(|r| async move {
            match r {
                Ok(Message::UiCommand(ui_command)) => Some(Ok(ui_command)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        })
    }

    pub fn event_stream(self) -> impl Stream<Item = Result<Event>> {
        self.stream().filter_map(|r| async move {
            match r {
                Ok(Message::Event(event)) => Some(Ok(event)),
                Err(e) => Some(Err(e)),
                _ => None,
            }
        })
    }

    fn translate_recv(r: Result<Message, RecvError>) -> Option<Message> {
        match r {
            Ok(message) => Some(message),
            Err(RecvError::Lagged(_)) => {
                panic!("MessageBus receiver lagged!");
            }
            Err(RecvError::Closed) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    UiUpdate(UiUpdate),
    UiCommand(UiCommand),
    Event(Event),
}

impl From<UiState> for Message {
    fn from(value: UiState) -> Self {
        Message::UiUpdate(UiUpdate::State(value))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UiUpdate {
    State(UiState),
    DiscordInfo(Vec<DiscordPlayerInfo>),
}

#[derive(Debug, Clone)]
pub struct MatchStart {
    pub game: GameId,
    // Sending full player db seems wasteful though any optimization here seems more wasteful
    pub player_db: PlayerDb,
    pub left_team: MatchStartTeam,
    pub right_team: MatchStartTeam,
}

#[derive(Debug, Clone)]
pub struct MatchStartTeam {
    // TODO(j): since we have full playerdb in MatchStart, maybe we shouldn't send elo here?
    pub players: HashMap<PlayerId, i32>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct RichMatchResult {
    pub winner_team_name: String,
    pub duration: Duration,
    pub scale: WinScale,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvatarUrl(String);

impl From<String> for AvatarUrl {
    fn from(value: String) -> Self {
        AvatarUrl(value)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordPlayerInfo {
    pub id: PlayerId,
    pub display_name: String,
    pub username: DiscordUsername,
    pub avatar_url: AvatarUrl,
}

#[derive(Debug, Clone, Copy)]
pub enum ImageFormat {
    Jpg,
    Bmp,
    Png,
}

impl ImageFormat {
    pub fn from_mime_type(mime: &str) -> Option<Self> {
        match mime {
            "image/jpg" => Some(ImageFormat::Jpg),
            "image/png" => Some(ImageFormat::Png),
            "image/bmp" => Some(ImageFormat::Bmp),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            ImageFormat::Jpg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Png => "png",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    MatchStart(MatchStart),
    RichMatchResult(RichMatchResult),
    DotaScreenshotReceived(Bytes, Option<ImageFormat>),
}

#[derive(Clone, Debug)]
pub enum UiCommand {
    InitializeUi,
    AddNewPlayer(Player),
    RemovePlayer(PlayerId),
    MovePlayerToOtherTeam(PlayerId),
    RemovePlayerFromTeam(PlayerId),
    AddPlayerToTeam(PlayerId, Team),
    AddPlayerToLobby(PlayerId),
    RemovePlayerFromLobby(PlayerId),
    ChangeGame(GameId),
    FillLobby,
    ClearLobby,
    CallToLobby,
    CallPlayer(PlayerId),
    StartMatch,
    ShuffleTeams,
    RefreshElo,
    FinishMatch(FinishMatch),
    AddLobbyScreenshotData(Vec<String>),
    CloseApplication,
}

#[derive(Clone, Debug)]
pub enum FinishMatch {
    Cancelled,
    Finished {
        winner: Team,
        scale: WinScale,
        duration: Duration,
        fake: bool,
    },
}
