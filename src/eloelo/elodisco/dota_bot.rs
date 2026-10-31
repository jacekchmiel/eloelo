use super::bot_state::DotaBotState;
use crate::eloelo::elodisco::config::EloDiscoConfig;
use crate::eloelo::elodisco::hero_assignment_strategy::{
    DotaTeam, HeroAssignmentStrategy, HeroAssignmentStrategyKind, PlayerInfo, RandomHeroPool,
    RoleAssignment, TaggedHeroPool,
};
use crate::eloelo::message_bus::{Event, HeroesAssigned, MatchStart, Message, MessageBus};
use crate::eloelo::options::HeroAssignmentOptions;
use anyhow::{format_err, Error, Result};
use chrono::{DateTime, Local};
use eloelo_model::player::{DiscordUsername, PlayerDb};
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
            .lines()
            .map(|s| Hero(String::from(s.trim().split(',').next().unwrap().trim())))
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

pub enum RerollResult {
    NewPool(Vec<Hero>),
    NoRerollUntil(DateTime<Local>),
}

fn make_hero_assign_strategy(
    options: &HeroAssignmentOptions,
) -> Box<dyn HeroAssignmentStrategy + Send + Sync> {
    match options.strategy {
        HeroAssignmentStrategyKind::Random => Box::new(RandomHeroPool::default()),
        HeroAssignmentStrategyKind::Tags => {
            Box::new(TaggedHeroPool::new(RoleAssignment::OrderedRotation))
        }
    }
}

pub struct DotaBot {
    state: HashMap<DiscordUsername, DotaBotState>,
    heroes: HashSet<Hero>,
    hero_assign_strategy: Box<dyn HeroAssignmentStrategy + Send + Sync>,
    message_bus: MessageBus,
    global_banned_heroes: HashSet<Hero>,
    unrestricted_players: HashSet<PlayerId>,
    /// Latest players received from eloelo.
    player_db: PlayerDb,
}

impl DotaBot {
    pub fn new(
        state: HashMap<DiscordUsername, DotaBotState>,
        player_db: PlayerDb,
        message_bus: MessageBus,
        config: &EloDiscoConfig,
    ) -> Self {
        if !config.global_banned_heroes.is_empty() {
            let banned_list: Vec<_> = config.global_banned_heroes.iter().map(|h| h.as_str()).collect();
            info!("Globally banned heroes: {}", banned_list.join(", "));
        }
        if !config.unrestricted_players.is_empty() {
            let unrestricted_list: Vec<_> = config
                .unrestricted_players
                .iter()
                .map(|p| p.as_str())
                .collect();
            info!(
                "Players not affected by global bans: {}",
                unrestricted_list.join(", ")
            );
        }
        DotaBot {
            state,
            message_bus,
            heroes: Hero::all(),
            hero_assign_strategy: make_hero_assign_strategy(&HeroAssignmentOptions::default()),
            global_banned_heroes: config.global_banned_heroes.clone(),
            unrestricted_players: config.unrestricted_players.clone(),
            player_db,
        }
    }

    pub async fn on_message(&mut self, message: &Message) {
        match message {
            Message::Event(Event::PlayerDbUpdated(player_db)) => {
                self.player_db = player_db.clone();
            }
            Message::Event(Event::MatchStart(match_start)) => {
                self.player_db = match_start.player_db.clone();
                let assignments = self.on_match_start(match_start).await;
                self.message_bus
                    .send(Message::Event(Event::HeroesAssigned(HeroesAssigned {
                        match_start: match_start.clone(),
                        assignments,
                        tags: self.hero_assign_strategy.assigned_tags(),
                    })))
            }
            _ => {}
        }
    }

    pub fn user_hero_pool(&self, username: &DiscordUsername) -> Vec<Hero> {
        let user_state = self.get_user_state(username);
        let no_bans = HashSet::new();
        let banned = if self.is_unrestricted(username) {
            &no_bans
        } else {
            &self.global_banned_heroes
        };
        let pool: Vec<_> = if !user_state.allowed_heroes.is_empty() {
            user_state.allowed_heroes
                .difference(banned)
                .cloned()
                .collect()
        } else {
            self.heroes
                .difference(&user_state.banned_heroes).cloned()
                .collect::<HashSet<_>>()
                .difference(banned)
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
                    .get_user_state(&discord_username)
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
        info!("Hero assignment: {:?}", match_start.hero_assignment);
        self.hero_assign_strategy = make_hero_assign_strategy(&match_start.hero_assignment);
        let hero_assignments = self.hero_assign_strategy.assign_heroes(hero_pools);

        hero_assignments
            .into_iter()
            .map(|(k, v)| (k.name.clone(), v))
            .collect()
    }

    pub fn reroll(&mut self, username: &DiscordUsername) -> Result<RerollResult> {
        let state = self.state.entry(username.clone()).or_default();
        let now = Local::now();
        cleanup_reroll_log(state, now);
        if state.reroll_log.len() as u32 >= state.reroll_limit_num {
            return Ok(RerollResult::NoRerollUntil(next_reroll_at(state, now)));
        }
        state.reroll_log.push(now);

        let hero_pool = self.user_hero_pool(username);
        debug!(
            "{} rerolled heroes. Remaining pool: {}",
            username,
            hero_pool.join(", ")
        );
        Ok(RerollResult::NewPool(
            self.hero_assign_strategy.reroll(username, hero_pool)?,
        ))
    }

    pub async fn get_state(&self) -> HashMap<DiscordUsername, DotaBotState> {
        self.state.clone()
    }

    pub fn player_db(&self) -> &PlayerDb {
        &self.player_db
    }

    /// Whether `username` belongs to one of `unrestricted_players`. A player without a Discord
    /// username is matched by id.
    fn is_unrestricted(&self, username: &DiscordUsername) -> bool {
        self.unrestricted_players.iter().any(|id| {
            match self.player_db.get(id).and_then(|p| p.discord_username()) {
                Some(discord_username) => discord_username == username,
                None => id.as_str() == username.as_str(),
            }
        })
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

fn next_reroll_at(state: &mut DotaBotState, now: DateTime<Local>) -> DateTime<Local> {
    match state.reroll_log.first() {
        Some(ts) => *ts + chrono::Duration::minutes(state.reroll_limit_duration_minutes as i64),
        None => now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eloelo_model::player::Player;

    #[tokio::test]
    async fn test_keeps_player_db_received_from_eloelo() {
        let config = EloDiscoConfig::default();
        let mut bot = DotaBot::new(
            HashMap::new(),
            PlayerDb::new([]),
            MessageBus::new(),
            &config,
        );
        let player = Player::with_opt_discord_username(
            PlayerId::from("j"),
            Some(DiscordUsername::from("powerwazniak")),
        );

        bot.on_message(&Message::Event(Event::PlayerDbUpdated(PlayerDb::new([
            player,
        ]))))
        .await;

        let player = bot.player_db().get(&PlayerId::from("j")).unwrap();
        assert_eq!(
            player.discord_username(),
            Some(&DiscordUsername::from("powerwazniak"))
        );
    }

    #[test]
    fn test_global_banned_heroes_excluded_from_pool() {
        let mut config = EloDiscoConfig::default();
        config.global_banned_heroes =
            [Hero::from("Phantom Lancer"), Hero::from("Medusa")].into_iter().collect();
        let message_bus = MessageBus::new();
        let bot = DotaBot::new(HashMap::new(), PlayerDb::new([]), message_bus, &config);
        let user = DiscordUsername::from("test_user");
        let pool = bot.user_hero_pool(&user);
        assert!(!pool.contains(&Hero::from("Phantom Lancer")));
        assert!(!pool.contains(&Hero::from("Medusa")));
    }

    #[test]
    fn test_global_banned_heroes_with_user_allowlist() {
        let mut config = EloDiscoConfig::default();
        config.global_banned_heroes =
            [Hero::from("Phantom Lancer"), Hero::from("Medusa")].into_iter().collect();
        let message_bus = MessageBus::new();
        let user = DiscordUsername::from("test_user");

        let mut user_state = DotaBotState::default();
        // User allowlist includes a globally banned hero ("Phantom Lancer") and other heroes
        user_state.allowed_heroes = [
            Hero::from("Phantom Lancer"),
            Hero::from("Axe"),
            Hero::from("Lion"),
        ]
        .into_iter()
        .collect();

        let mut state = HashMap::new();
        state.insert(user.clone(), user_state);

        let bot = DotaBot::new(state, PlayerDb::new([]), message_bus, &config);
        let pool = bot.user_hero_pool(&user);

        // Global ban takes precedence over user allowlist
        assert!(!pool.contains(&Hero::from("Phantom Lancer")));
        // Remaining allowed heroes are present
        assert!(pool.contains(&Hero::from("Axe")));
        assert!(pool.contains(&Hero::from("Lion")));
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_global_banned_heroes_with_user_banlist() {
        let mut config = EloDiscoConfig::default();
        config.global_banned_heroes =
            [Hero::from("Phantom Lancer"), Hero::from("Medusa")].into_iter().collect();
        let message_bus = MessageBus::new();
        let user = DiscordUsername::from("test_user");

        let mut user_state = DotaBotState::default();
        // User bans Axe
        user_state.banned_heroes = [Hero::from("Axe")].into_iter().collect();

        let mut state = HashMap::new();
        state.insert(user.clone(), user_state);

        let bot = DotaBot::new(state, PlayerDb::new([]), message_bus, &config);
        let pool = bot.user_hero_pool(&user);

        // Both globally banned heroes and user-banned heroes are excluded
        assert!(!pool.contains(&Hero::from("Phantom Lancer")));
        assert!(!pool.contains(&Hero::from("Medusa")));
        assert!(!pool.contains(&Hero::from("Axe")));

        // Non-banned heroes are present in pool
        assert!(pool.contains(&Hero::from("Lion")));
        assert_eq!(pool.len(), Hero::all().len() - 3);
    }

    #[test]
    fn test_unrestricted_players_ignore_global_bans() {
        let mut config = EloDiscoConfig::default();
        config.global_banned_heroes = [Hero::from("Phantom Lancer"), Hero::from("Medusa")]
            .into_iter()
            .collect();
        config.unrestricted_players = [PlayerId::from("j"), PlayerId::from("hypys")]
            .into_iter()
            .collect();
        let player_db = PlayerDb::new([
            Player::with_opt_discord_username(
                PlayerId::from("j"),
                Some(DiscordUsername::from("powerwazniak")),
            ),
            Player::with_opt_discord_username(PlayerId::from("hypys"), None),
            Player::with_opt_discord_username(
                PlayerId::from("Spawek"),
                Some(DiscordUsername::from("spawek")),
            ),
        ]);
        let bot = DotaBot::new(HashMap::new(), player_db, MessageBus::new(), &config);

        // Matched by Discord username
        let pool = bot.user_hero_pool(&DiscordUsername::from("powerwazniak"));
        assert!(pool.contains(&Hero::from("Phantom Lancer")));
        assert!(pool.contains(&Hero::from("Medusa")));
        assert_eq!(pool.len(), Hero::all().len());

        // Player without Discord username is matched by id
        let pool = bot.user_hero_pool(&DiscordUsername::from("hypys"));
        assert_eq!(pool.len(), Hero::all().len());

        // The id doesn't match a player who has a different Discord username
        let pool = bot.user_hero_pool(&DiscordUsername::from("j"));
        assert!(!pool.contains(&Hero::from("Phantom Lancer")));

        // Other players are still affected
        let pool = bot.user_hero_pool(&DiscordUsername::from("spawek"));
        assert!(!pool.contains(&Hero::from("Phantom Lancer")));
        assert!(!pool.contains(&Hero::from("Medusa")));
    }
}

