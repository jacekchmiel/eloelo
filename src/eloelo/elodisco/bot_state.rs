use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local};
use eloelo_model::player::DiscordUsername;
use log::warn;
use serde::{Deserialize, Serialize};

use super::dota_bot::Hero;

//TODO: either introduce separate DiscordUsername type or make PlayerId == username + add display name
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BotState {
    pub players: HashMap<DiscordUsername, PlayerBotState>,
}

impl BotState {
    /// Removes unknown heroes from players' allow and ban lists.
    pub fn validate(&mut self) {
        let all_heroes = Hero::all();
        for (username, player) in &mut self.players {
            retain_valid_heroes(&mut player.dota.banned_heroes, &all_heroes, username, "banned_heroes");
            retain_valid_heroes(&mut player.dota.allowed_heroes, &all_heroes, username, "allowed_heroes");
        }
    }
}

fn retain_valid_heroes(
    heroes: &mut HashSet<Hero>,
    all_heroes: &HashSet<Hero>,
    username: &DiscordUsername,
    list_name: &str,
) {
    heroes.retain(|hero| {
        let valid = all_heroes.contains(hero);
        if !valid {
            warn!("Discarding unknown hero \"{hero}\" from {list_name} of {username}");
        }
        valid
    });
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

    /// Reroll limit x per duration. X component.
    #[serde(default = "default_reroll_limit_x")]
    pub reroll_limit_num: u32,

    /// Reroll limit x per duration. Duration component.
    #[serde(default = "default_reroll_limit_duration_minutes")]
    pub reroll_limit_duration_minutes: u32,

    #[serde(default)]
    pub reroll_log: Vec<DateTime<Local>>,
}

fn default_num_heroes_shown() -> u32 {
    3
}

fn default_reroll_limit_x() -> u32 {
    1
}

fn default_reroll_limit_duration_minutes() -> u32 {
    // Period is per one-two games
    80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_discards_unknown_heroes() {
        let yaml = r#"
players:
  someuser:
    notifications: true
    dota:
      banned_heroes: [Pudge, Not A Hero]
      allowed_heroes: [Drow Ranger, drow ranger]
"#;
        let mut state: BotState = serde_yaml::from_str(yaml).unwrap();
        state.validate();
        let dota = &state.players[&DiscordUsername::from(String::from("someuser"))].dota;
        assert_eq!(dota.banned_heroes, HashSet::from([Hero::from("Pudge")]));
        assert_eq!(dota.allowed_heroes, HashSet::from([Hero::from("Drow Ranger")]));
    }
}
