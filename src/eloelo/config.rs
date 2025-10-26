use std::collections::HashSet;
use std::path::PathBuf;

use eloelo_model::{GameId, PlayerId, Team};
use serde::{Deserialize, Serialize};

use super::store;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub games: Vec<Game>,

    #[serde(default)]
    pub discord_bot_token: String,

    #[serde(default)]
    pub discord_server_name: String,

    #[serde(default)]
    pub discord_channel_name: String,

    /// Disables priv messages and storing history. Useful for development and testing.
    #[serde(default)]
    pub test_mode: bool,

    #[serde(default)]
    pub discord_test_channel_name: String,

    /// List of usernames that will receive notifications even in test mode.
    #[serde(default)]
    pub discord_test_mode_players: HashSet<PlayerId>,

    #[serde(default = "default_hero_assign_algo")]
    pub hero_assign_algo: AssignAlgo,

    #[serde(default = "default_history_git_mirror")]
    pub history_git_mirror: PathBuf,

    #[serde(default)]
    pub dota_screenshot_dir: Option<PathBuf>,

    #[serde(default = "default_fosiaudio_host")]
    pub fosiaudio_host: String,

    #[serde(default = "default_fosiaudio_timeout_ms")]
    pub fosiaudio_timeout_ms: u64,

    #[serde(default = "default_dota_ocr_engine_command")]
    pub dota_ocr_engine_command: String,

    #[serde(default = "default_dota_ocr_engine_pwd")]
    pub dota_ocr_engine_pwd: Option<PathBuf>,

    #[serde(default = "default_static_serving_dir")]
    pub static_serving_dir: PathBuf,

    #[serde(default = "default_serving_addr")]
    pub serving_addr: String,
}

fn default_serving_addr() -> String {
    "0.0.0.0:3000".into()
}

fn default_static_serving_dir() -> PathBuf {
    "ui/dist".into()
}

fn default_dota_ocr_engine_pwd() -> Option<PathBuf> {
    Some(PathBuf::from("dota-ocr-engine"))
}

fn default_dota_ocr_engine_command() -> String {
    "uv run dota-ocr-engine.py %".into()
}

fn default_fosiaudio_timeout_ms() -> u64 {
    3 * 1000
}

fn default_hero_assign_algo() -> AssignAlgo {
    AssignAlgo::Random
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
            discord_bot_token: Default::default(),
            discord_server_name: Default::default(),
            discord_channel_name: Default::default(),
            hero_assign_algo: default_hero_assign_algo(),
            history_git_mirror: default_history_git_mirror(),
            dota_screenshot_dir: None,
            fosiaudio_host: default_fosiaudio_host(),
            fosiaudio_timeout_ms: default_fosiaudio_timeout_ms(),
            dota_ocr_engine_command: default_dota_ocr_engine_command(),
            dota_ocr_engine_pwd: default_dota_ocr_engine_pwd(),
            static_serving_dir: default_static_serving_dir(),
            serving_addr: default_serving_addr(),
            test_mode: true,
            discord_test_mode_players: Default::default(),
            discord_test_channel_name: Default::default(),
        }
    }
}

impl Config {
    pub fn default_game(&self) -> &GameId {
        self.games.first().map(|g| &g.name).unwrap()
    }

    pub fn effective_discord_channel_name(&self) -> &str {
        if self.test_mode {
            &self.discord_test_channel_name
        } else {
            &self.discord_channel_name
        }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AssignAlgo {
    Random,
    Tags,
}
