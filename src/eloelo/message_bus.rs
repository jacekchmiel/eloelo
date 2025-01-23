use anyhow::Result;
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

    pub fn blocking_recv(&mut self) -> Option<Message> {
        Self::translate_recv(self.0.blocking_recv())
    }

    pub fn ui_update_stream(self) -> impl Stream<Item = Result<UiUpdate>> {
        BroadcastStream::new(self.0).filter_map(|r| async move {
            match r {
                Ok(Message::UiUpdate(ui_update)) => Some(Ok(ui_update)),
                Err(broadcast_err) => Some(Err(broadcast_err.into())),
                _ => None,
            }
        })
    }

    pub fn ui_command_stream(self) -> impl Stream<Item = Result<UiCommand>> {
        BroadcastStream::new(self.0).filter_map(|r| async move {
            match r {
                Ok(Message::UiCommand(ui_command)) => Some(Ok(ui_command)),
                Err(broadcast_err) => Some(Err(broadcast_err.into())),
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

impl Message {
    pub fn try_into_ui_update(self) -> Option<UiUpdate> {
        match self {
            Message::UiUpdate(ui_update) => Some(ui_update),
            _ => None,
        }
    }

    pub fn try_into_ui_command(self) -> Option<UiCommand> {
        match self {
            Message::UiCommand(ui_command) => Some(ui_command),
            _ => None,
        }
    }
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
    RoleRecommendation(Vec<RoleRecommendation>),
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
pub struct RoleRecommendation {
    pub id: PlayerId,
    pub username: DiscordUsername, // TODO: Probably keep only one of the ids (the more convenient
    // from producer perspective
    pub recommendation: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordPlayerInfo {
    pub id: PlayerId,
    pub display_name: String,
    pub username: DiscordUsername,
    pub avatar_url: AvatarUrl,
}

#[derive(Debug, Clone)]
pub enum Event {
    MatchStart(MatchStart),
    RichMatchResult(RichMatchResult),
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
