use std::time::Duration;
use std::{borrow::Borrow, collections::HashMap};

use chrono::{DateTime, Local};
use log::error;
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

impl History {
    pub fn calculate_lose_streaks(
        &self,
        game: &GameId,
        players: impl Iterator<Item = impl Borrow<PlayerId>>,
    ) -> HashMap<PlayerId, i32> {
        let Some(entries) = self.entries.get(game) else {
            error!("Missing history entries to calculate lose streaks for requested game: {game}");
            return Default::default();
        };

        let rev_entries = entries.iter().rev().filter(|e| !e.fake);
        players
            .map(|p| {
                (
                    p.borrow().clone(),
                    rev_entries
                        .clone()
                        .take_while(|e| !e.winner.contains(p.borrow()))
                        .filter(|e| e.loser.contains(p.borrow()))
                        .count() as i32,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn make_entry(
        time: i64,
        winner: impl IntoIterator<Item = &'static str>,
        loser: impl IntoIterator<Item = &'static str>,
    ) -> HistoryEntry {
        HistoryEntry {
            timestamp: DateTime::<Utc>::from_timestamp(time, 0).unwrap().into(),
            winner: winner.into_iter().map(PlayerId::from).collect(),
            loser: loser.into_iter().map(PlayerId::from).collect(),
            scale: WinScale::Even,
            duration: Duration::from_secs(40 * 60),
            fake: false,
        }
    }

    #[test]
    fn calculate_lose_streaks_test() {
        let game_id = GameId::from("game");
        let history = History {
            entries: HashMap::from([(
                game_id.clone(),
                vec![
                    make_entry(1, ["bixkog", "spawek"], ["j"]),
                    make_entry(2, ["bixkog", "spawek"], ["j", "hypys"]),
                    make_entry(3, ["bixkog"], ["j", "bania", "hypys"]),
                ],
            )]),
        };
        let players = ["bixkog", "spawek", "j", "hypys", "bania"]
            .into_iter()
            .map(PlayerId::from);
        let streaks = history.calculate_lose_streaks(&game_id, players);
        assert_eq!(streaks.get(&PlayerId::from("j")).copied(), Some(3));
        assert_eq!(streaks.get(&PlayerId::from("hypys")).copied(), Some(2));
        assert_eq!(streaks.get(&PlayerId::from("bania")).copied(), Some(1));
        assert_eq!(streaks.get(&PlayerId::from("spawek")).copied(), Some(0));
        assert_eq!(streaks.get(&PlayerId::from("bixkog")).copied(), Some(0));
    }
}
