use super::bot_state::DotaBotState;
use crate::eloelo::config::Config;
use crate::eloelo::elodisco::hero_assignment_strategy::{
    DotaTeam, HeroAssignmentStrategy, PlayerInfo, RandomHeroPool, TaggedHeroPool,
};
use crate::eloelo::message_bus::{Event, HeroesAssigned, MatchStart, Message, MessageBus};
use anyhow::{format_err, Error, Result};
use chrono::{DateTime, Local};
use eloelo_model::player::DiscordUsername;
use eloelo_model::PlayerId;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hero(String);

impl Hero {
    pub fn all() -> HashSet<Hero> {
        include_str!("dota_heroes.csv")
            .split("\n")
            .map(|s| Hero(String::from(s.split(',').next().unwrap().trim())))
            .collect()
    }

    pub fn all_alphabetical() -> Vec<Hero> {
        let mut heroes: Vec<_> = Hero::all().into_iter().collect();
        heroes.sort();
        heroes
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
    hero_assign_strategy: Box<dyn HeroAssignmentStrategy + Send + Sync>,
    message_bus: MessageBus,
}

impl DotaBot {
    pub fn new(
        state: HashMap<DiscordUsername, DotaBotState>,
        message_bus: MessageBus,
        config: &Config,
    ) -> Self {
        DotaBot {
            state,
            message_bus,
            heroes: Hero::all(),
            hero_assign_strategy: match config.hero_assignment_strategy {
                crate::eloelo::config::HeroAssignmentStrategyKind::Random => {
                    info!("HeroAssignment strategy: Random");
                    Box::new(RandomHeroPool::default())
                }

                crate::eloelo::config::HeroAssignmentStrategyKind::Tags => {
                    info!("HeroAssignment strategy: Tags");
                    Box::new(TaggedHeroPool::new())
                }
            },
        }
    }

    pub async fn on_message(&mut self, message: &Message) {
        match message {
            Message::Event(Event::MatchStart(match_start)) => {
                let assignments = self.on_match_start(match_start).await;
                self.message_bus
                    .send(Message::Event(Event::HeroesAssigned(HeroesAssigned {
                        match_start: match_start.clone(),
                        assignments,
                    })))
            }
            _ => {}
        }
    }

    pub fn user_hero_pool(&self, username: &DiscordUsername) -> Vec<Hero> {
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
        let create_player_info = |id: &PlayerId, elo: i32, dota_team: DotaTeam| -> PlayerInfo {
            let discord_username: DiscordUsername = match_start
                .player_db
                .get(id)
                .unwrap()
                .discord_username()
                .unwrap_or(&DiscordUsername::from(id.as_str()))
                .clone();
            PlayerInfo {
                elo,
                number_of_heroes_shown: self
                    .state
                    .get(&discord_username)
                    .expect("discord user state")
                    .num_heroes_shown,
                dota_team,
                name: discord_username,
            }
        };
        match_start
            .left_team
            .players
            .iter()
            .map(|(id, elo)| create_player_info(id, *elo, DotaTeam::Radiant))
            .chain(
                match_start
                    .right_team
                    .players
                    .iter()
                    .map(|(id, elo)| create_player_info(id, *elo, DotaTeam::Dire)),
            )
            .into_iter()
            .map(|u| {
                let pool = self.user_hero_pool(&u.name);
                (u, pool)
            })
            .collect()
    }

    async fn on_match_start(
        &mut self,
        match_start: &MatchStart,
    ) -> HashMap<DiscordUsername, Vec<Hero>> {
        let hero_pools = self.make_hero_pools(&match_start);
        self.hero_assign_strategy.clear();
        let hero_assignments = self.hero_assign_strategy.assign_heroes(hero_pools);

        hero_assignments
            .into_iter()
            .map(|(k, v)| (k.name.clone(), v))
            .collect()
    }

    pub fn reroll(&mut self, username: &DiscordUsername) -> Result<Vec<Hero>> {
        let state = self.state.entry(username.clone()).or_default();
        let now = Local::now();
        cleanup_reroll_log(state, now);
        if state.reroll_log.len() as u32 >= state.reroll_limit_num {
            // FIXME: return timestamp when reroll will be available again
            return Ok(Vec::new());
        }
        state.reroll_log.push(now);

        let hero_pool = self.user_hero_pool(username);
        debug!(
            "{} rerolled heroes. Remaining pool: {}",
            username,
            hero_pool.join(", ")
        );
        self.hero_assign_strategy.reroll(username, hero_pool)
    }

    pub async fn get_state(&self) -> HashMap<DiscordUsername, DotaBotState> {
        self.state.clone()
    }

    pub fn get_user_state(&self, user: &DiscordUsername) -> DotaBotState {
        self.state.get(user).cloned().unwrap_or_default()
    }

    pub fn ban_hero(&mut self, username: &DiscordUsername, hero: &Hero) {
        info!("{username} banned {hero}");
        if let Some(s) = self.state.get_mut(username) {
            s.banned_heroes.insert(hero.clone());
        };
    }

    pub fn unban_hero(&mut self, username: &DiscordUsername, hero: &Hero) {
        info!("{username} unbanned {hero}");
        if let Some(s) = self.state.get_mut(username) {
            s.banned_heroes.remove(hero);
        };
    }
    pub fn allow_hero(&mut self, username: &DiscordUsername, hero: &Hero) {
        info!("{username} allowed {hero}");
        if let Some(s) = self.state.get_mut(username) {
            s.allowed_heroes.insert(hero.clone());
        };
    }
    pub fn unallow_hero(&mut self, username: &DiscordUsername, hero: &Hero) {
        info!("{username} unallowed {hero}");
        if let Some(s) = self.state.get_mut(username) {
            s.allowed_heroes.remove(hero);
        };
    }
    pub fn clear_allowlist(&mut self, username: &DiscordUsername) {
        info!("{username} cleared allowlist");
        if let Some(s) = self.state.get_mut(username) {
            s.allowed_heroes.clear();
        };
    }
    pub fn clear_banlist(&mut self, username: &DiscordUsername) {
        info!("{username} cleared banlist");
        if let Some(s) = self.state.get_mut(username) {
            s.banned_heroes.clear();
        };
    }
}

fn cleanup_reroll_log(state: &mut DotaBotState, now: DateTime<Local>) {
    let irrelevancy_horizon =
        now - chrono::Duration::minutes(state.reroll_limit_duration_minutes.into());
    let mut old_log = Vec::new();
    std::mem::swap(&mut old_log, &mut state.reroll_log);

    state.reroll_log = old_log
        .into_iter()
        .filter(|t| *t > irrelevancy_horizon)
        .collect();
}
