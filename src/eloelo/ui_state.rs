use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use spawelo::SpaweloOptions;

use super::config::Game;
use eloelo_model::history::History;
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
}

impl State {
    pub fn new(selected_game: GameId) -> Self {
        Self {
            selected_game,
            left_team: Default::default(),
            right_team: Default::default(),
            game_state: Default::default(),
            lobby: Default::default(),
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
    pub pity_bonus: f32,
}

impl From<&BalancedTeam> for TeamPityBonus {
    fn from(value: &BalancedTeam) -> Self {
        TeamPityBonus {
            pity_elo: value.pity_elo,
            pity_bonus: value.pity_bonus,
            real_elo: value.real_elo,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub available_games: Vec<Game>,

    pub selected_game: GameId,

    pub left_players: Vec<UiPlayer>,
    pub right_players: Vec<UiPlayer>,
    pub reserve_players: Vec<UiPlayer>,

    pub pity_bonus: PityBonus,

    pub game_state: GameState,

    pub history: History,
    pub options: SpaweloOptions,
}
