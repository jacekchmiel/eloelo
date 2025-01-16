use std::path::PathBuf;

use eloelo_model::player::{DiscordUsername, Player};
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
        }
    }
}

impl Config {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerConfig {
    #[serde(alias = "name")] // TODO(j): Legacy field name, remove in 2025
    pub id: PlayerId,
    pub display_name: Option<String>,
    pub discord_username: Option<DiscordUsername>,
    pub fosiaudio_name: Option<String>,
}

impl From<PlayerConfig> for Player {
    fn from(value: PlayerConfig) -> Self {
        Player {
            id: value.id,
            display_name: value.display_name,
            discord_username: value.discord_username,
            elo: Default::default(),
            fosiaudio_name: value.fosiaudio_name,
        }
    }
}

impl From<Player> for PlayerConfig {
    fn from(value: Player) -> Self {
        PlayerConfig {
            id: value.id,
            display_name: value.display_name,
            discord_username: value.discord_username,
            fosiaudio_name: value.fosiaudio_name,
        }
    }
}
