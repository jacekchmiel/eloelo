use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Local};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{GameId, PlayerId, WinScale};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub entries: HashMap<GameId, Vec<HistoryEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Local>,
    pub winner: Vec<PlayerId>,
    pub loser: Vec<PlayerId>,
    #[serde(default)]
    pub scale: WinScale,
    #[serde(default = "default_match_duration")]
    #[serde(serialize_with = "serialize_seconds")]
    #[serde(deserialize_with = "deserialize_seconds")]
    pub duration: Duration,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub fake: bool,
}

fn is_default<T: Default + PartialEq<T>>(v: &T) -> bool {
    v == &Default::default()
}

fn default_match_duration() -> Duration {
    Duration::from_secs(45 * 60)
}

fn serialize_seconds<S: Serializer>(duration: &Duration, s: S) -> Result<S::Ok, S::Error> {
    duration.as_secs().serialize(s)
}

fn deserialize_seconds<'de, D>(d: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds = u64::deserialize(d)?;
    Ok(Duration::from_secs(seconds))
}

impl HistoryEntry {
    pub fn all_players(&self) -> impl Iterator<Item = &PlayerId> {
        self.winner.iter().chain(self.loser.iter())
    }

    pub fn advantage_factor(&self) -> f64 {
        match self.scale {
            WinScale::Even => 0.75,
            WinScale::Advantage => 0.85,
            WinScale::Pwnage => 0.95,
        }
    }
}
