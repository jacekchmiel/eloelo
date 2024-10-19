use std::collections::HashMap;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::{GameId, PlayerId};

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
        }
    }
}
