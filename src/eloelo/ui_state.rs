use std::collections::{HashMap, HashSet};

use eloelo_model::decimal::Decimal;
use eloelo_model::options::DescribedOptionsGroup;
use serde::{Deserialize, Serialize};

use super::config::Game;
use eloelo_model::history::HistoryEntry;
use eloelo_model::{BalancedTeam, GameId, GameState, PlayerId};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub selected_game: GameId,

    #[serde(default)]
    pub left_team: BalancedTeam,
    #[serde(default)]
    pub right_team: BalancedTeam,

    #[serde(default)]
    pub game_state: GameState,

    #[serde(default)]
    pub lobby: HashSet<PlayerId>,

    #[serde(default)]
    pub shuffle_temperature: i32,
}

impl State {
    pub fn new(selected_game: GameId) -> Self {
        Self {
            selected_game,
            left_team: Default::default(),
            right_team: Default::default(),
            game_state: Default::default(),
            lobby: Default::default(),
            shuffle_temperature: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPlayer {
    pub id: PlayerId,
    pub name: String,
    pub discord_username: Option<String>,
    pub elo: i32,
    pub present_in_lobby: bool,
    pub lose_streak: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PityBonus {
    pub left: TeamPityBonus,
    pub right: TeamPityBonus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPityBonus {
    pub real_elo: i32,
    pub pity_elo: i32,
    #[serde(default)]
    pub pity_bonus_mul: f64,
    #[serde(default)]
    pub pity_bonus_add: i32,
}

impl From<&BalancedTeam> for TeamPityBonus {
    fn from(value: &BalancedTeam) -> Self {
        TeamPityBonus {
            pity_elo: value.pity_elo,
            pity_bonus_mul: value.pity_bonus_mul,
            pity_bonus_add: value.pity_bonus_add,
            real_elo: value.real_elo,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiHistory {
    pub entries: HashMap<GameId, Vec<UiHistoryEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiHistoryEntry {
    pub entry: HistoryEntry,
    pub metadata: MatchMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchMetadata {
    pub winner_elo: i32,
    pub loser_elo: i32,
    pub winner_chance: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub available_games: Vec<Game>,

    pub selected_game: GameId,

    pub left_players: Vec<UiPlayer>,
    pub right_players: Vec<UiPlayer>,
    pub reserve_players: Vec<UiPlayer>,
    pub win_prediction: Decimal,
    pub shuffle_temperature: i32,

    pub pity_bonus: PityBonus,

    pub game_state: GameState,

    pub history: UiHistory,
    pub options: Vec<DescribedOptionsGroup>,
}
