use std::collections::HashMap;

use anyhow::Error;
use eloelo_model::player::DiscordUsername;

use crate::eloelo::elodisco::dota_bot::Hero;

mod random_hero_pool;
mod tagged_hero_pool;
mod tests;

pub use random_hero_pool::RandomHeroPool;
pub use tagged_hero_pool::TaggedHeroPool;

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub enum DotaTeam {
    Radiant,
    Dire,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct PlayerInfo {
    pub name: DiscordUsername,
    pub elo: i32,
    pub dota_team: DotaTeam,
    pub number_of_heroes_shown: u32,
}

pub trait HeroAssignmentStrategy {
    fn assign_heroes(
        &mut self,
        hero_pools: Vec<(PlayerInfo, Vec<Hero>)>,
    ) -> HashMap<PlayerInfo, Vec<Hero>>;

    fn reroll(
        &mut self,
        player: &DiscordUsername,
        hero_pool: Vec<Hero>,
    ) -> Result<Vec<Hero>, Error>;

    fn clear(&mut self);
}
