use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::dota_bot::Hero;

//TODO: either introduce separate DiscordUsername type or make PlayerId == username + add display name
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BotState {
    pub players: HashMap<String, PlayerBotState>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerBotState {
    pub notifications: bool,
    pub dota: DotaBotState,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DotaBotState {
    /// Controls whether send random hero selection on match start.
    pub randomizer: bool,

    /// List of banned heroes that won't show up in randomizer.
    pub banned_heroes: HashSet<Hero>,

    /// List of allowed heroes. If not empty, randomizer will use only this pool.
    pub allowed_heroes: HashSet<Hero>,
}
