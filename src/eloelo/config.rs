use std::path::PathBuf;

use eloelo_model::{GameId, Team};
use serde::{Deserialize, Serialize};

/// eloelo's config. The Discord bot has its own, `EloDiscoConfig`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub games: Vec<Game>,

    /// Disables priv messages, storing match history and syncing the history git mirror. Finished
    /// matches are kept in memory only. Useful for development and testing. The Discord bot
    /// follows the value of the connected eloelo.
    #[serde(default)]
    pub test_mode: bool,

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

    #[serde(default)]
    pub discord_bot: DiscordBotMode,
}

/// Where eloelo's Discord bot runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum DiscordBotMode {
    /// In the eloelo process.
    #[default]
    Embedded,
    /// In a separate elodisco process. eloelo connects to it at `url`, e.g. `ws://host:3001`.
    Remote { url: String },
    Disabled,
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

fn default_fosiaudio_host() -> String {
    "127.0.0.1:1234".into()
}
impl Default for Config {
    fn default() -> Self {
        Self {
            games: vec![Game::new(GameId::from("Default Game"))],
            dota_screenshot_dir: None,
            fosiaudio_host: default_fosiaudio_host(),
            fosiaudio_timeout_ms: default_fosiaudio_timeout_ms(),
            dota_ocr_engine_command: default_dota_ocr_engine_command(),
            dota_ocr_engine_pwd: default_dota_ocr_engine_pwd(),
            static_serving_dir: default_static_serving_dir(),
            serving_addr: default_serving_addr(),
            discord_bot: Default::default(),
            test_mode: true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_fields_of_other_configs_and_secrets() {
        let config: Config = serde_yaml::from_str("fosiaudioHost: host").unwrap();
        assert_eq!(config.fosiaudio_host, "host");
        for yaml in [
            "bridgeSecret: secret",
            "discordBotToken: token",
            "discordServerName: x",
        ] {
            assert!(serde_yaml::from_str::<Config>(yaml).is_err(), "{yaml}");
        }
    }
}
