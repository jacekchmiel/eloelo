use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use crate::eloelo::message_bus::MatchStart;
use crate::eloelo::print_err;
use eloelo_model::player::DiscordUsername;

use super::bot_state::DotaBotState;
use super::command_handler::{CommandDescription, CommandHandler};
use anyhow::{format_err, Context as _, Error, Result};
use log::{error, info};
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
        include_str!("dota_heroes.txt")
            .split("\n")
            .map(|s| Hero(String::from(s.trim())))
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

pub struct DotaBot {
    state: HashMap<DiscordUsername, DotaBotState>,
    heroes: HashSet<Hero>,
}

impl DotaBot {
    pub fn with_state(state: HashMap<DiscordUsername, DotaBotState>) -> Self {
        Self {
            state,
            heroes: Hero::all(),
        }
    }

    pub fn get_state(&self) -> &HashMap<DiscordUsername, DotaBotState> {
        &self.state
    }

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

    fn random_heroes_str(&self, player: &DiscordUsername) -> String {
        self.random_heroes(player)
            .into_iter()
            .map(|h| h.0.clone())
            .collect::<Vec<_>>()
            .join(",\n")
    }

    pub async fn match_start(
        &self,
        match_start: &MatchStart,
        ctx: &Context,
        members: &HashMap<DiscordUsername, User>,
    ) {
        let players = match_start
            .left_team
            .players
            .keys()
            .chain(match_start.right_team.players.keys());
        let discord_users = players
            .flat_map(|p| match_start.player_db.get(p))
            .flat_map(|p| &p.discord_username);
        for username in discord_users {
            let notifications_enabled = self
                .state
                .get(username)
                .map(|s| s.randomizer)
                .unwrap_or(false);
            if notifications_enabled {
                let heroes_message = format!(
                    "**Your random heroes for this match are**\n{}",
                    self.random_heroes_str(username)
                );
                match members.get(&username) {
                    Some(user) => {
                        let _ = user
                            .dm(&ctx, CreateMessage::new().content(heroes_message))
                            .await
                            .context("dota heroes notification")
                            .inspect_err(print_err);
                    }
                    None => error!("{} not found in guild members", username),
                }
            }
        }
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
                info!("{} enabled hero randomization", username);
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
            DotaCommand::Hero => Some(Ok(self.random_heroes_str(username))),
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
        }
    }
}

fn heroes_str(heroes: &HashSet<Hero>) -> Option<String> {
    if heroes.is_empty() {
        return None;
    }
    let heroes: Vec<&str> = heroes.iter().map(|h| h.as_str()).collect();
    Some(heroes.join(",\n"))
}
