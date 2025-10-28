use std::collections::{HashMap, HashSet};

use anyhow::{format_err, Error};
use eloelo_model::player::DiscordUsername;
use itertools::Itertools;
use rand::seq::SliceRandom;

use crate::eloelo::elodisco::{
    dota_bot::Hero,
    hero_assignment_strategy::{HeroAssignmentStrategy, PlayerInfo},
};

#[derive(Default)]
pub struct RandomHeroPool {
    pub hero_assignement: HashMap<PlayerInfo, Vec<Hero>>,
    pub taken: HashSet<Hero>,
}

impl HeroAssignmentStrategy for RandomHeroPool {
    fn assign_heroes(
        &mut self,
        mut hero_pools: Vec<(PlayerInfo, Vec<Hero>)>,
    ) -> HashMap<PlayerInfo, Vec<Hero>> {
        // Sort players by pool length
        hero_pools.sort_by_key(|v| (v.1.len(), rand::random::<i32>()));
        let max_num_of_heroes = hero_pools
            .iter()
            .map(|v| v.0.number_of_heroes_shown)
            .max()
            .unwrap_or(3);
        // Select one hero per player, banning the selected hero along the way
        // (Ignore when a player's pool is empty)
        for _ in 0..max_num_of_heroes {
            for (player, hero_pool) in hero_pools.iter_mut() {
                if self
                    .hero_assignement
                    .get(player)
                    .map(|v| v.len() == (player.number_of_heroes_shown as usize))
                    .is_some_and(|x| x == true)
                {
                    continue;
                }
                hero_pool.shuffle(&mut rand::thread_rng());
                while let Some(hero) = hero_pool.pop() {
                    if self.taken.contains(&hero) {
                        continue;
                    }
                    self.taken.insert(hero.clone());
                    self.hero_assignement
                        .entry(player.clone())
                        .or_default()
                        .push(hero);
                    break;
                }
            }
        }
        self.hero_assignement.clone()
    }

    fn reroll(
        &mut self,
        username: &DiscordUsername,
        hero_pool: Vec<Hero>,
    ) -> Result<Vec<Hero>, Error> {
        if self.hero_assignement.is_empty() {
            return Ok(Vec::new());
        }
        let Some(player) = self
            .hero_assignement
            .keys()
            .find(|p| p.name == *username)
            .cloned()
        else {
            return Err(format_err!(
                "Player {} is not in the current game.",
                *username
            ));
        };
        let old_picks = self.hero_assignement.get(&player).unwrap().clone();
        self.hero_assignement.get_mut(&player).unwrap().clear();
        let mut assignement =
            self.assign_heroes(vec![(player.clone(), hero_pool.into_iter().collect_vec())]);
        let new_picks = assignement.entry(player.clone()).or_default().to_vec();
        self.taken = self
            .taken
            .union(&new_picks.iter().cloned().collect())
            .collect::<HashSet<_>>()
            .difference(&old_picks.iter().collect())
            .map(|h| (*h).clone())
            .collect();
        self.hero_assignement
            .entry(player.clone())
            .insert_entry(new_picks.clone());
        Ok(new_picks)
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}
