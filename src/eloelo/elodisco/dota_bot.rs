use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use crate::eloelo::config::Config;
use crate::eloelo::elodisco::hero_pool_generators::{
    HeroPoolGenerator, PlayerInfo, RandomHeroPool, TaggedHeroPool,
};
use crate::eloelo::elodisco::utils::send_direct_message;
use crate::eloelo::message_bus::MatchStart;
use crate::utils;
use chrono::Local;
use eloelo_model::player::DiscordUsername;
use eloelo_model::PlayerId;

use super::bot_state::DotaBotState;
use super::command_handler::{CommandDescription, CommandHandler};
use anyhow::{format_err, Error, Result};
use log::{debug, error, info};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serenity::all::{Context, CreateMessage, User};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hero(String);

impl Hero {
    pub fn try_from_args(args: &[&str]) -> Result<Self> {
        Self::try_from(args.join(" "))
    }

    pub fn all() -> HashSet<Hero> {
        include_str!("dota_heroes.csv")
            .split("\n")
            .map(|s| Hero(String::from(s.split(',').next().unwrap().trim())))
            .collect()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Hero {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if Hero::all().contains(value.as_str()) {
            Ok(Hero(value))
        } else {
            Err(format_err!(
                "Incorrect hero name: \"{}\". See `all` for list of valid names.",
                value
            ))
        }
    }
}

#[cfg(test)]
impl From<&str> for Hero {
    fn from(value: &str) -> Self {
        Hero::try_from(String::from(value)).unwrap()
    }
}

impl Display for Hero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Borrow<str> for Hero {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<Hero> for Hero {
    fn as_ref(&self) -> &Hero {
        &self
    }
}

pub struct DotaBot {
    state: HashMap<DiscordUsername, DotaBotState>,
    heroes: HashSet<Hero>,
    assign_algo: Box<dyn HeroPoolGenerator + Send + Sync>,

    pub discord_test_mode: bool,
    pub discord_test_mode_players: HashSet<PlayerId>,
}

impl DotaBot {
    pub fn with_state(state: HashMap<DiscordUsername, DotaBotState>) -> Self {
        Self {
            state,
            heroes: Hero::all(),
            assign_algo: Box::new(RandomHeroPool::default()),
            discord_test_mode: false,
            discord_test_mode_players: Default::default(),
        }
    }

    pub fn configure(mut self, config: &Config) -> Self {
        self.discord_test_mode = config.test_mode;
        self.discord_test_mode_players = config.discord_test_mode_players.clone();
        match config.hero_assign_algo {
            crate::eloelo::config::AssignAlgo::Random => {
                self.assign_algo = Box::new(RandomHeroPool::default())
            }
            crate::eloelo::config::AssignAlgo::Tags => {
                self.assign_algo = Box::new(TaggedHeroPool::new())
            }
        }
        self
    }

    pub fn get_state(&self) -> &HashMap<DiscordUsername, DotaBotState> {
        &self.state
    }

    /// Just computes random heroes without checking for conflicts
    fn random_heroes(&self, player: &DiscordUsername) -> Vec<Hero> {
        let default_config = DotaBotState::default();
        let player_config = self.state.get(player).unwrap_or(&default_config);
        let mut hero_pool: Vec<_> = if !player_config.allowed_heroes.is_empty() {
            player_config.allowed_heroes.iter().collect()
        } else {
            self.heroes
                .iter()
                .filter(|h| !player_config.banned_heroes.contains(*h))
                .collect()
        };

        hero_pool.shuffle(&mut rand::thread_rng());
        hero_pool.into_iter().take(3).cloned().collect()
    }

    fn random_heroes_str(&self, heroes: impl IntoIterator<Item = impl AsRef<Hero>>) -> String {
        heroes
            .into_iter()
            .map(|h| h.as_ref().0.clone())
            .collect::<Vec<_>>()
            .join(",\n")
    }

    fn user_hero_pool(&self, username: &DiscordUsername) -> Vec<Hero> {
        let user_state = self.state.get(username).expect("discord user state");
        let pool: Vec<_> = if !user_state.allowed_heroes.is_empty() {
            user_state.allowed_heroes.iter().cloned().collect()
        } else {
            self.heroes
                .difference(&user_state.banned_heroes)
                .cloned()
                .collect()
        };
        let should_avoid_duplicates = match user_state.last_match_date {
            Some(date) => Local::now() < date + chrono::Duration::days(1),
            None => false,
        };
        let has_enough_heroes_to_avoid_duplicates = pool.len() >= 3;
        if should_avoid_duplicates
            && has_enough_heroes_to_avoid_duplicates
            && !user_state.duplicate_heroes_opt_out
        {
            pool.into_iter()
                .filter(|h| !user_state.last_match_heroes.contains(h.as_str()))
                .collect()
        } else {
            pool
        }
    }

    fn make_hero_pools(&self, match_start: &MatchStart) -> Vec<(PlayerInfo, Vec<Hero>)> {
        debug!("dota bot state: {:?}", self.state);
        let create_player_info = |id: &PlayerId, elo: i32, is_radiant: bool| -> PlayerInfo {
            let discord_username = match_start
                .player_db
                .get(id)
                .unwrap()
                .discord_username()
                .unwrap();
            debug!("{}", discord_username);
            PlayerInfo {
                elo,
                number_of_heroes_shown: self
                    .state
                    .get(discord_username)
                    .expect("discord user state")
                    .num_of_heroes_shown as u32,
                is_radiant,
                name: discord_username.clone(),
            }
        };
        match_start
            .left_team
            .players
            .iter()
            .map(|(id, elo)| create_player_info(id, *elo, true))
            .chain(
                match_start
                    .right_team
                    .players
                    .iter()
                    .map(|(id, elo)| create_player_info(id, *elo, false)),
            )
            .into_iter()
            .map(|u| {
                let pool = self.user_hero_pool(&u.name);
                (u, pool)
            })
            .collect()
    }

    pub async fn match_start(
        &mut self,
        match_start: &MatchStart,
        ctx: &Context,
        members: &HashMap<DiscordUsername, User>,
    ) -> HashMap<DiscordUsername, Vec<Hero>> {
        let players = match_start
            .left_team
            .players
            .keys()
            .chain(match_start.right_team.players.keys());
        let discord_users = players
            .flat_map(|p| match_start.player_db.get(p))
            .flat_map(|p| p.discord_username().map(|u| (&p.id, u)));
        let with_randomizer: Vec<(&PlayerId, &DiscordUsername)> = discord_users
            .filter(|u| self.state.get(u.1).map(|s| s.randomizer).unwrap_or(false))
            .collect();
        let should_notify: HashSet<_> = with_randomizer
            .iter()
            .filter(|(p, _)| self.is_allowed_to_receive_notifications(p))
            .map(|(_, u)| u)
            .copied()
            .collect();
        let hero_pools = self.make_hero_pools(&match_start);
        self.assign_algo.clear();
        let hero_assignments = self.assign_algo.assign_heroes(hero_pools);

        let hero_notifications = hero_assignments
            .iter()
            .filter(|(user, _)| should_notify.contains(&user.name));
        for (user, heroes) in hero_notifications {
            info!(
                "Hero assignment {}: {}",
                user.name,
                utils::join(heroes, ", ")
            );
            let heroes_message = format!(
                "**Your random heroes for this match are**\n{}",
                self.random_heroes_str(heroes)
            );
            // TODO: parallelize sending messages

            match members.get(&user.name) {
                Some(user) => {
                    let message = CreateMessage::new().content(heroes_message);
                    send_direct_message(ctx.clone(), user.clone(), message, "heroes").await;
                }
                None => error!(
                    "{} not found in guild members. This should not happen.",
                    user.name
                ),
            }
        }
        hero_assignments
            .into_iter()
            .map(|(k, v)| (k.name.clone(), v.into_iter().collect()))
            .collect()
    }

    pub fn reroll(&mut self, username: &DiscordUsername) -> Vec<Hero> {
        let hero_pool = self.user_hero_pool(username);
        self.assign_algo.reroll_user_heroes(username, hero_pool)
    }

    pub fn is_allowed_to_receive_notifications(&self, p: &PlayerId) -> bool {
        !self.discord_test_mode || self.discord_test_mode_players.contains(p)
    }
}

enum DotaCommand {
    EnableRandom,
    Banned,
    Allowed,
    Hero,
    All,
    Ban(Hero),
    Unban(Hero),
    Allow(Hero),
    Unallow(Hero),
    Reroll,
}

impl DotaCommand {
    pub fn try_from_cmd_and_args(command: &str, args: &[&str]) -> Result<Option<DotaCommand>> {
        let command = match command {
            "enable-random" => Some(DotaCommand::EnableRandom),
            "banned" => Some(DotaCommand::Banned),
            "allowed" => Some(DotaCommand::Allowed),
            "hero" => Some(DotaCommand::Hero),
            "all" => Some(DotaCommand::All),
            "ban" => Some(DotaCommand::Ban(Hero::try_from_args(args)?)),
            "unban" => Some(DotaCommand::Unban(Hero::try_from_args(args)?)),
            "allow" => Some(DotaCommand::Allow(Hero::try_from_args(args)?)),
            "unallow" => Some(DotaCommand::Unallow(Hero::try_from_args(args)?)),
            "reroll" => Some(DotaCommand::Reroll),
            _ => None,
        };
        Ok(command)
    }
}

impl CommandHandler for DotaBot {
    fn supported_commands(&self) -> Vec<CommandDescription> {
        return vec![
            CommandDescription {
                keyword: "enable-random".into(),
                description: "Enable random hero notifications".into(),
            },
            CommandDescription {
                keyword: "banned".into(),
                description: "Show banned heroes".into(),
            },
            CommandDescription {
                keyword: "allowed".into(),
                description: "Show allowed heroes".into(),
            },
            CommandDescription {
                keyword: "hero".into(),
                description: "Responds with random hero choices".into(),
            },
            CommandDescription {
                keyword: "ban".into(),
                description: "Add hero to your ban list. E.g. `ban Chen`".into(),
            },
            CommandDescription {
                keyword: "allow".into(),
                description: "Add hero to your allow list".into(),
            },
            CommandDescription {
                keyword: "unban".into(),
                description: "Remove hero from your allow list. E.g. `unban Chen`".into(),
            },
            CommandDescription {
                keyword: "unallow".into(),
                description: "Remove hero from your allow list".into(),
            },
            CommandDescription {
                keyword: "clear".into(),
                description: "Clear your allow/ban list depending on argument. E.g. `clear ban`."
                    .into(),
            },
            CommandDescription {
                keyword: "all".into(),
                description: "List all heroes".into(),
            },
            CommandDescription {
                keyword: "reroll".into(),
                description: "Rerolls your heroes in the current game.".into(),
            },
        ];
    }

    fn dispatch_command(
        &mut self,
        username: &DiscordUsername,
        command: &str,
        args: &[&str],
    ) -> Option<Result<String>> {
        let command = match DotaCommand::try_from_cmd_and_args(command, args) {
            Ok(Some(command)) => command,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };
        match command {
            DotaCommand::All => Some(Ok(
                heroes_str(&self.heroes).unwrap_or_else(|| String::from("No heroes loaded :["))
            )),
            DotaCommand::EnableRandom => {
                self.state
                    .entry(username.clone())
                    .or_insert(Default::default())
                    .randomizer = true;
                info!("{} enabled hero rgandomization", username);
                Some(Ok(String::from("Hero randomization enabled.")))
            }
            DotaCommand::Banned => Some(Ok(self
                .state
                .get(username)
                .and_then(|s| heroes_str(&s.banned_heroes))
                .unwrap_or_else(|| String::from("No banned heroes.")))),
            DotaCommand::Allowed => Some(Ok(self
                .state
                .get(username)
                .and_then(|s| heroes_str(&s.allowed_heroes))
                .unwrap_or_else(|| String::from("All heroes allowed (except bans).")))),
            DotaCommand::Hero => Some(Ok(self.random_heroes_str(&self.random_heroes(&username)))),
            DotaCommand::Ban(hero) => {
                if let Some(s) = self.state.get_mut(username) {
                    s.banned_heroes.insert(hero.clone());
                };
                Some(Ok(format!("{} is now banned.", hero).into()))
            }
            DotaCommand::Unban(hero) => {
                if let Some(s) = self.state.get_mut(username) {
                    s.banned_heroes.remove(&hero);
                }
                Some(Ok(format!("{} is now unbanned.", hero).into()))
            }
            DotaCommand::Allow(hero) => {
                if let Some(s) = self.state.get_mut(username) {
                    s.allowed_heroes.insert(hero.clone());
                };
                Some(Ok(format!("{} is now allowed.", hero).into()))
            }
            DotaCommand::Unallow(hero) => {
                if let Some(s) = self.state.get_mut(username) {
                    s.allowed_heroes.remove(&hero);
                }
                Some(Ok(format!("{} is now unallowed.", hero).into()))
            }
            DotaCommand::Reroll => {
                let new_pool = self.reroll(username);
                if new_pool.is_empty() {
                    Some(Ok(String::from("Can't reroll heroes.")))
                } else {
                    Some(Ok(format!(
                        "**Your random heroes for this match are**\n{}",
                        self.random_heroes_str(new_pool)
                    )))
                }
            }
        }
    }
}

fn heroes_str(heroes: &HashSet<Hero>) -> Option<String> {
    if heroes.is_empty() {
        return None;
    }
    let mut heroes: Vec<&str> = heroes.iter().map(|h| h.as_str()).collect();
    heroes.sort_unstable();
    Some(format!(
        "{}\n\nThats {} total heroes",
        heroes.join(",\n"),
        heroes.len()
    ))
}
