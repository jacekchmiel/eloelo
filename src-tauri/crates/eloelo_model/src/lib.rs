use std::fmt::Display;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WinScale {
    Even,
    Advantage,
    Pwnage,
}

impl Default for WinScale {
    fn default() -> Self {
        WinScale::Even
    }
}

#[derive(Error, Debug)]
#[error("Invalid value: {0}")]
pub struct FromStrError(String);

impl TryFrom<&str> for WinScale {
    type Error = FromStrError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "even" => Ok(WinScale::Even),
            "advantage" => Ok(WinScale::Advantage),
            "pwnage" => Ok(WinScale::Pwnage),
            other => Err(FromStrError(other.to_string())),
        }
    }
}

impl TryFrom<String> for WinScale {
    type Error = FromStrError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        WinScale::try_from(value.as_str())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_str_error() {
        assert_eq!(
            &WinScale::try_from("domination").unwrap_err().to_string(),
            "Invalid value: domination"
        );
    }
}
