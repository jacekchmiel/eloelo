use std::path::PathBuf;

use eloelo_model::player::PlayerConfig;
use eloelo_model::{GameId, PlayerId, Team};
use serde::{Deserialize, Serialize};

use super::store;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub games: Vec<Game>,

    #[serde(default)]
    pub players: Vec<PlayerConfig>,

    #[serde(default)]
    pub discord_bot_token: String,

    #[serde(default)]
    pub discord_server_name: String,

    #[serde(default)]
    pub discord_channel_name: String,

    #[serde(default)]
    pub max_elo_history: usize,

    #[serde(default = "default_history_git_mirror")]
    pub history_git_mirror: PathBuf,

    #[serde(default)]
    pub dota_screenshot_dir: Option<PathBuf>,

    #[serde(default = "default_fosiaudio_host")]
    pub fosiaudio_host: String,

    #[serde(default = "default_fosiaudio_timeout_ms")]
    pub fosiaudio_timeout_ms: u64,
}

fn default_fosiaudio_timeout_ms() -> u64 {
    3 * 1000
}

fn default_history_git_mirror() -> PathBuf {
    store::data_dir().join("history_git")
}

fn default_fosiaudio_host() -> String {
    "127.0.0.1:1234".into()
}
impl Default for Config {
    fn default() -> Self {
        Self {
            games: vec![Game::new(GameId::from("Default Game"))],
            players: vec![],
            discord_bot_token: Default::default(),
            discord_server_name: Default::default(),
            discord_channel_name: Default::default(),
            max_elo_history: 0,
            history_git_mirror: default_history_git_mirror(),
            dota_screenshot_dir: None,
            fosiaudio_host: default_fosiaudio_host(),
            fosiaudio_timeout_ms: default_fosiaudio_timeout_ms(),
        }
    }
}

impl Config {
    pub fn get_player(&self, p: &PlayerId) -> Option<&PlayerConfig> {
        self.players.iter().find(|v| v.id == *p)
    }

    pub fn default_game(&self) -> &GameId {
        self.games.first().map(|g| &g.name).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub name: GameId,
    #[serde(default = "left_team_default")]
    pub left_team: String,
    #[serde(default = "right_team_default")]
    pub right_team: String,
}

impl Game {
    pub fn new(name: GameId) -> Self {
        Game {
            name,
            left_team: left_team_default(),
            right_team: right_team_default(),
        }
    }

    pub fn team_name(&self, t: Team) -> &str {
        match t {
            Team::Left => &self.left_team,
            Team::Right => &self.right_team,
        }
    }
}

fn left_team_default() -> String {
    "Left Team".into()
}

fn right_team_default() -> String {
    "Right Team".into()
}
