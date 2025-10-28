use std::{
    collections::{HashMap, HashSet},
    convert::identity,
    iter::zip,
    str::FromStr,
};

use anyhow::{format_err, Error, Result};
use eloelo_model::player::DiscordUsername;
use itertools::Itertools;
use log::{info, warn};
use rand::seq::{IteratorRandom, SliceRandom};

use crate::eloelo::elodisco::hero_assignment_strategy::DotaTeam;
use crate::eloelo::elodisco::{
    dota_bot::Hero,
    hero_assignment_strategy::{HeroAssignmentStrategy, PlayerInfo},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeroTag {
    Carry,
    Core,
    Support,
}

impl HeroTag {
    pub fn next_tag(tag_id: usize) -> HeroTag {
        vec![HeroTag::Core, HeroTag::Support, HeroTag::Carry][tag_id % 3].clone()
    }
}

impl FromStr for HeroTag {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "Carry" => Ok(HeroTag::Carry),
            "Core" => Ok(HeroTag::Core),
            "Support" => Ok(HeroTag::Support),
            _ => Err(format_err!("Invalid hero tag: {}", s)),
        }
    }
}

#[derive(Default, Clone)]
pub struct TaggedHeroPool {
    pub tags: HashMap<HeroTag, HashSet<Hero>>,
    pub hero_assignement: HashMap<PlayerInfo, Vec<Hero>>,
    pub taken: HashSet<Hero>,
}

impl TaggedHeroPool {
    pub fn new() -> Self {
        TaggedHeroPool {
            tags: Self::read_tags(),
            ..Default::default()
        }
    }

    fn read_tags() -> HashMap<HeroTag, HashSet<Hero>> {
        include_str!("../dota_heroes.csv")
            .split("\n")
            .map(|s| {
                let raw: Vec<&str> = s.split(",").map(|s| s.trim()).collect();
                (raw[0].to_string(), raw[1].to_string())
            })
            .fold(HashMap::new(), |mut m, (hero_name, tag_name)| {
                if let Some(hero) = Hero::try_from(hero_name).ok() {
                    match HeroTag::from_str(tag_name.as_str()) {
                        Ok(tag) => {
                            m.entry(tag).or_default().insert(hero);
                        }
                        Err(e) => panic!("Error when reading hero tags: {}", e),
                    }
                }
                m
            })
    }

    pub fn deduce_tag(&self, hero: &Hero) -> HeroTag {
        for (tag, heroes) in self.tags.iter() {
            if heroes.contains(hero) {
                return tag.clone();
            }
        }
        warn!("Can't deduce tag for hero: {}, returning support!", hero);
        HeroTag::Support
    }

    fn assign_random_hero(&mut self, hero_pool: &HashSet<Hero>) -> Option<Hero> {
        hero_pool
            .difference(&self.taken)
            .choose(&mut rand::thread_rng())
            .cloned()
    }

    fn assign_tagged_hero(&mut self, hero_pool: &HashSet<Hero>, tag: &HeroTag) -> Option<Hero> {
        let mut heroes = self.tags.get(tag).unwrap().clone();
        heroes = heroes
            .difference(&self.taken)
            .cloned()
            .collect::<HashSet<Hero>>()
            .intersection(hero_pool)
            .cloned()
            .collect();
        if heroes.len() == 0 {
            info!("Fallback to random hero assignement.");
            return self.assign_random_hero(hero_pool); // fallback to random hero from players pool
        }
        heroes.into_iter().choose(&mut rand::thread_rng())
    }

    fn players_pairing(t1_len: usize, t2_len: usize) -> Vec<usize> {
        let [t_min, t_max] = if t2_len < t1_len {
            [t2_len, t1_len]
        } else {
            [t1_len, t2_len]
        };
        let mut pairing_order = (0..t_min).collect_vec();
        pairing_order.shuffle(&mut rand::thread_rng());
        pairing_order.into_iter().chain(t_min..t_max).collect()
    }
}

impl HeroAssignmentStrategy for TaggedHeroPool {
    fn assign_heroes(
        &mut self,
        hero_pools: Vec<(PlayerInfo, Vec<Hero>)>,
    ) -> HashMap<PlayerInfo, Vec<Hero>> {
        let hero_pools_sets = hero_pools
            .into_iter()
            .map(|(name, heroes)| (name, HashSet::from_iter(heroes.iter().cloned())))
            .collect_vec();
        let radiant = hero_pools_sets
            .iter()
            .filter(|(p, _)| p.dota_team == DotaTeam::Radiant)
            .sorted_by_key(|(p, _)| -p.elo)
            .collect_vec();
        let dire = hero_pools_sets
            .iter()
            .filter(|(p, _)| p.dota_team == DotaTeam::Dire)
            .sorted_by_key(|(p, _)| -p.elo)
            .collect_vec();
        let max_hero_shown = hero_pools_sets
            .iter()
            .map(|(p, _)| p.number_of_heroes_shown)
            .max()
            .unwrap_or(3);
        let pairing_order = Self::players_pairing(radiant.len(), dire.len());
        let pairs_tag = (0..pairing_order.len())
            .map(HeroTag::next_tag)
            .collect_vec(); // rotate Core -> Support -> Carry -> ... for balanced team composition
        for _ in 0..max_hero_shown {
            for (pair_id, pair_tag) in zip(pairing_order.iter(), pairs_tag.iter()) {
                for team in [&radiant, &dire] {
                    if team.len() <= *pair_id {
                        // not even number of players per team
                        continue;
                    }
                    let (player, hero_pool) = team[*pair_id];
                    if self
                        .hero_assignement
                        .get(player)
                        .and_then(|v| Some(v.len() == (player.number_of_heroes_shown as usize)))
                        .is_some_and(identity)
                    {
                        continue;
                    }
                    if let Some(pick) = self.assign_tagged_hero(hero_pool, pair_tag) {
                        self.hero_assignement
                            .entry(player.clone())
                            .or_default()
                            .push(pick.clone());
                        self.taken.insert(pick);
                    }
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
        let player_tag = self.deduce_tag(&self.hero_assignement.get(&player).unwrap()[0]);
        let hero_pool_set = HashSet::from_iter(hero_pool.into_iter());
        let new_picks = (0..player.number_of_heroes_shown)
            .filter_map(|_| {
                self.assign_tagged_hero(&hero_pool_set, &player_tag) // old heroes are in self.taken so they will not be rerolled
            })
            .collect_vec();
        let old_picks = self.hero_assignement.get(&player).unwrap().clone();
        self.taken = self
            .taken
            .union(&new_picks.iter().cloned().collect())
            .collect::<HashSet<_>>()
            .difference(&old_picks.iter().collect())
            .map(|h| (*h).clone())
            .collect();
        self.hero_assignement
            .entry(player)
            .insert_entry(new_picks.clone());
        Ok(new_picks)
    }

    fn clear(&mut self) {
        self.hero_assignement.clear();
        self.taken.clear();
    }
}
