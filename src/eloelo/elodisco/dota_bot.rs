use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use crate::eloelo::elodisco::utils::send_direct_message;
use crate::eloelo::message_bus::MatchStart;
use crate::utils;
use eloelo_model::player::DiscordUsername;

use super::bot_state::DotaBotState;
use super::command_handler::{CommandDescription, CommandHandler};
use anyhow::{format_err, Error, Result};
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

    /// Assigns random heroes in a way that minimizes chances for conflicts
    fn assign_random_heroes<'a>(
        &self,
        users: &'a [DiscordUsername],
    ) -> HashMap<&'a DiscordUsername, Vec<&Hero>> {
        let mut hero_pools: Vec<(&DiscordUsername, Vec<&Hero>)> = self.make_hero_pools(users);
        // Sort players by pool length
        hero_pools.sort_by_key(|v| (v.1.len(), rand::random::<i32>())); // TODO: randomize order of same-size users

        // Select one hero per player, banning the selected hero along the way
        // (Ignore when a player's pool is empty)
        let mut taken: HashSet<&Hero> = HashSet::new();
        let mut selections: HashMap<&DiscordUsername, Vec<&Hero>> = HashMap::new();
        for _ in 0..3 {
            for (user, hero_pool) in &mut hero_pools {
                hero_pool.shuffle(&mut rand::thread_rng());
                while let Some(hero) = hero_pool.pop() {
                    if taken.contains(hero) {
                        continue;
                    }
                    taken.insert(hero);
                    selections.entry(user).or_default().push(hero);
                    break;
                }
            }
        }
        selections
    }

    fn make_hero_pools<'a>(
        &self,
        users: &'a [DiscordUsername],
    ) -> Vec<(&'a DiscordUsername, Vec<&Hero>)> {
        users
            .into_iter()
            .map(|u| {
                let user_state = self.state.get(u).expect("discord user state");
                if !user_state.allowed_heroes.is_empty() {
                    (u, user_state.allowed_heroes.iter().collect())
                } else {
                    let pool = self.heroes.difference(&user_state.banned_heroes).collect();
                    (u, pool)
                }
            })
            .collect()
    }

    pub async fn match_start(
        &self,
        match_start: &MatchStart,
        ctx: &Context,
        members: &HashMap<DiscordUsername, User>,
    ) -> HashMap<DiscordUsername, Vec<&Hero>> {
        let players = match_start
            .left_team
            .players
            .keys()
            .chain(match_start.right_team.players.keys());
        let discord_users = players
            .flat_map(|p| match_start.player_db.get(p))
            .flat_map(|p| &p.discord_username);
        let users_with_randomizer: Vec<DiscordUsername> = discord_users
            .filter(|u| self.state.get(u).map(|s| s.randomizer).unwrap_or(false))
            .cloned()
            .collect();

        let mut hero_assignments: Vec<_> = self
            .assign_random_heroes(&users_with_randomizer)
            .into_iter()
            .collect();
        hero_assignments.sort_by_key(|u| u.0);

        for (username, heroes) in &hero_assignments {
            info!("Hero assignment {username}: {}", utils::join(heroes, ", "));
            let heroes_message = format!(
                "**Your random heroes for this match are**\n{}",
                self.random_heroes_str(heroes)
            );
            // TODO: parallelize sending messages
            match members.get(&username) {
                Some(user) => {
                    let message = CreateMessage::new().content(heroes_message);
                    tokio::spawn(send_direct_message(ctx.clone(), user.clone(), message));
                }
                None => error!(
                    "{} not found in guild members. This should not happen.",
                    username
                ),
            }
        }
        hero_assignments
            .into_iter()
            .map(|(k, v)| (k.clone(), v))
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 1_000;

    fn player_j() -> DiscordUsername {
        DiscordUsername::from("j".to_string())
    }

    fn player_bixkog() -> DiscordUsername {
        DiscordUsername::from("bixkog".to_string())
    }

    #[test]
    fn random_from_allowed_pool() {
        let state = [(
            player_j(),
            DotaBotState {
                randomizer: true,
                banned_heroes: Default::default(),
                allowed_heroes: make_heroes(&["Sniper", "Axe", "Lina"]),
            },
        )]
        .into_iter()
        .collect();
        let bot = DotaBot::with_state(state);
        let users = [player_j()];
        for _ in 0..N {
            let assignments = bot.assign_random_heroes(&users);
            assert!(assignment_in(
                assignments.get(&player_j()).unwrap(),
                &make_heroes(&["Sniper", "Axe", "Lina"])
            ));
        }
    }

    #[test]
    fn random_with_banned() {
        let state = [(
            player_j(),
            DotaBotState {
                randomizer: true,
                banned_heroes: make_heroes(&["Sniper", "Axe", "Lina"]),
                allowed_heroes: Default::default(),
            },
        )]
        .into_iter()
        .collect();
        let bot = DotaBot::with_state(state);
        let users = [player_j()];
        for _ in 0..N {
            let assignments = bot.assign_random_heroes(&users);
            assert!(!assignment_in(
                assignments.get(&player_j()).unwrap(),
                &make_heroes(&["Sniper", "Axe", "Lina"])
            ));
        }
    }

    #[test]
    fn assignments_dont_overlap() {
        let state = [
            (
                player_bixkog(),
                DotaBotState {
                    randomizer: true,
                    banned_heroes: Default::default(),
                    allowed_heroes: Default::default(),
                },
            ),
            (
                player_j(),
                DotaBotState {
                    randomizer: true,
                    banned_heroes: Default::default(),
                    allowed_heroes: Default::default(),
                },
            ),
        ]
        .into_iter()
        .collect();
        let bot = DotaBot::with_state(state);
        let users = [player_j(), player_bixkog()];
        for _ in 0..N {
            let assignments = bot.assign_random_heroes(&users);
            assert!(no_overlaps(&assignments));
        }
    }

    fn make_heroes(names: &[&str]) -> HashSet<Hero> {
        names.into_iter().copied().map(Hero::from).collect()
    }

    fn no_overlaps(assignments: &HashMap<&DiscordUsername, Vec<&Hero>>) -> bool {
        let total_len = assignments.values().flatten().count();
        let unique_len = assignments.values().flatten().collect::<HashSet<_>>().len();
        total_len == unique_len
    }

    fn assignment_in(assignment: &[&Hero], set: &HashSet<Hero>) -> bool {
        set.difference(&assignment.into_iter().map(|v| (*v).clone()).collect())
            .count()
            == 0
    }
}
