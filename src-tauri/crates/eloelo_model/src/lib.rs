use std::fmt::Display;

use serde::{Deserialize, Serialize};

pub mod history;
pub mod player;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GameState {
    #[default]
    AssemblingTeams,
    MatchInProgress,
}

#[derive(Copy, Clone, Debug)]
pub enum Team {
    Left,
    Right,
}

impl Team {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "left" => Some(Team::Left),
            "right" => Some(Team::Right),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct PlayerId(String);

impl PlayerId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl From<String> for PlayerId {
    fn from(value: String) -> Self {
        PlayerId(value)
    }
}

impl From<&str> for PlayerId {
    fn from(value: &str) -> Self {
        PlayerId(String::from(value))
    }
}

impl From<PlayerId> for String {
    fn from(value: PlayerId) -> Self {
        value.0
    }
}

impl Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct GameId(String);

impl Display for GameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl GameId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for GameId {
    fn from(value: String) -> Self {
        GameId(value)
    }
}

impl From<&str> for GameId {
    fn from(value: &str) -> Self {
        GameId(value.to_string())
    }
}
