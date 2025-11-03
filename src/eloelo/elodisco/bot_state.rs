use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local};
use eloelo_model::player::DiscordUsername;
use serde::{Deserialize, Serialize};

use super::dota_bot::Hero;

//TODO: either introduce separate DiscordUsername type or make PlayerId == username + add display name
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BotState {
    pub players: HashMap<DiscordUsername, PlayerBotState>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerBotState {
    pub notifications: bool,
    pub dota: DotaBotState,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DotaBotState {
    /// List of banned heroes that won't show up in randomizer.
    pub banned_heroes: HashSet<Hero>,

    /// List of allowed heroes. If not empty, randomizer will use only this pool.
    pub allowed_heroes: HashSet<Hero>,

    // Number of heroes drafted for the player per roll
    #[serde(default = "default_num_heroes_shown")]
    pub num_heroes_shown: u32,

    /// List of heroes offered last match.
    #[serde(default)]
    pub last_match_heroes: HashSet<Hero>,

    /// Date of the last match.
    #[serde(default)]
    pub last_match_date: Option<DateTime<Local>>,

    /// Explicit opt-out from duplicate heroes avoidance. With this set to true,
    /// the algorithm won't try to avoid presenting same hero twice in a row.
    #[serde(default)]
    pub duplicate_heroes_opt_out: bool,
}

fn default_num_heroes_shown() -> u32 {
    3
}
