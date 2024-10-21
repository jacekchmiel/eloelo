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
    pub win_probability: f64,
    #[serde(default)]
    pub scale: WinScale,
    #[serde(default = "default_match_duration")]
    #[serde(serialize_with = "serialize_seconds")]
    #[serde(deserialize_with = "deserialize_seconds")]
    pub duration: Duration,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LegacyHistoryEntry {
    pub teams: [Vec<PlayerId>; 2],
    pub winner: i32,
    pub win_probability: f64,
}

impl From<LegacyHistoryEntry> for HistoryEntry {
    fn from(value: LegacyHistoryEntry) -> Self {
        let [mut winner, mut loser] = value.teams;
        if value.winner != 0 {
            std::mem::swap(&mut winner, &mut loser);
        }
        let win_probability = value.win_probability;

        HistoryEntry {
            timestamp: DateTime::from(DateTime::UNIX_EPOCH),
            winner,
            loser,
            win_probability,
            scale: WinScale::Even,
            duration: Duration::from_secs(45 * 60),
        }
    }
}
