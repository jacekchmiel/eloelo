use std::collections::HashMap;
use std::time::Duration;

use eloelo_model::player::{DiscordUsername, Player, PlayerDb};
use eloelo_model::{GameId, PlayerId, Team, WinScale};
use log::error;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender};

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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum Event {
    MatchStart(MatchStart),
}

#[derive(Clone, Debug)]
pub enum UiCommand {
    InitializeUi,
    AddNewPlayer(Player),
    RemovePlayer(PlayerId),
    MovePlayerToOtherTeam(String),
    RemovePlayerFromTeam(String),
    AddPlayerToTeam(String, Team),
    ChangeGame(GameId),
    StartMatch,
    ShuffleTeams,
    RefreshElo,
    FinishMatch(FinishMatch),
    CloseApplication,
}

#[derive(Clone, Debug)]
pub enum FinishMatch {
    Cancelled,
    Finished {
        winner: Team,
        scale: WinScale,
        duration: Duration,
    },
}
