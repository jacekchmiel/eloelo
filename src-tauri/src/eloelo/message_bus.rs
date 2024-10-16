use std::collections::HashMap;

use eloelo_model::{GameId, PlayerId};
use log::error;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::UiCommand;

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
    Avatars(Vec<AvatarUpdate>),
}

#[derive(Debug, Clone)]
pub struct MatchStart {
    pub game: GameId,
    pub left_team: MatchStartTeam,
    pub right_team: MatchStartTeam,
}

#[derive(Debug, Clone)]
pub struct MatchStartTeam {
    pub players: HashMap<PlayerId, i32>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvatarUrl(pub String);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarUpdate {
    pub player: PlayerId,
    pub avatar_url: Option<AvatarUrl>,
}

#[derive(Debug, Clone)]
pub enum Event {
    MatchStart(MatchStart),
}
