use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use spawelo::SpaweloOptions;

use super::config::Game;
use eloelo_model::history::History;
use eloelo_model::{GameId, GameState, PlayerId};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub selected_game: GameId,

    #[serde(default)]
    pub left_players: Vec<PlayerId>,
    #[serde(default)]
    pub right_players: Vec<PlayerId>,

    #[serde(default)]
    pub game_state: GameState,

    #[serde(default)]
    pub lobby: HashSet<PlayerId>,
}

impl State {
    pub fn new(selected_game: GameId) -> Self {
        Self {
            selected_game,
            left_players: Default::default(),
            right_players: Default::default(),
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub available_games: Vec<Game>,

    pub selected_game: GameId,

    pub left_players: Vec<UiPlayer>,
    pub right_players: Vec<UiPlayer>,
    pub reserve_players: Vec<UiPlayer>,

    pub game_state: GameState,

    pub history: History,
    pub options: SpaweloOptions,
}
