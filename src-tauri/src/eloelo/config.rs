use eloelo_model::player::{DiscordUsername, Player};
use eloelo_model::{GameId, PlayerId};
use serde::{Deserialize, Serialize};

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
}

impl From<PlayerConfig> for Player {
    fn from(value: PlayerConfig) -> Self {
        Player {
            id: value.id,
            display_name: value.display_name,
            discord_username: value.discord_username,
            elo: Default::default(),
        }
    }
}

impl From<Player> for PlayerConfig {
    fn from(value: Player) -> Self {
        PlayerConfig {
            id: value.id,
            display_name: value.display_name,
            discord_username: value.discord_username,
        }
    }
}
