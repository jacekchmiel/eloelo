use std::collections::HashMap;

use anyhow::Error;
use eloelo_model::options::ChoiceOption;
use eloelo_model::player::DiscordUsername;
use serde::{Deserialize, Serialize};

use crate::eloelo::elodisco::dota_bot::Hero;

mod random_hero_pool;
mod role_assignment;
mod tagged_hero_pool;
mod tests;

pub use random_hero_pool::RandomHeroPool;
pub use role_assignment::RoleAssignment;
pub use tagged_hero_pool::{HeroTag, TaggedHeroPool};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HeroAssignmentStrategyKind {
    Random,
    #[default]
    Tags,
}

impl ChoiceOption for HeroAssignmentStrategyKind {
    fn choices() -> Vec<(Self, &'static str)> {
        vec![
            (HeroAssignmentStrategyKind::Random, "Random"),
            (
                HeroAssignmentStrategyKind::Tags,
                "Roles (Carry/Core/Support)",
            ),
        ]
    }
}

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

    /// Tags assigned to players in the current match. Empty for strategies that don't use tags.
    fn assigned_tags(&self) -> HashMap<DiscordUsername, HeroTag> {
        HashMap::new()
    }
}
