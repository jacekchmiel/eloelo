use core::f64;
use std::{
    collections::{HashMap, HashSet},
    iter::zip,
    str::FromStr,
};

use anyhow::{bail, format_err, Context, Error, Result};
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
    pub hero_similarity: HashMap<Hero, Vec<Hero>>,
}

impl TaggedHeroPool {
    pub fn new() -> Self {
        TaggedHeroPool {
            tags: Self::read_tags(),
            hero_similarity: Self::read_hero_similarity().unwrap(),
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

    fn read_hero_similarity() -> Result<HashMap<Hero, Vec<Hero>>, Error> {
        let csv_str = include_str!("hero_similarity_matrix.csv");
        let mut reader = csv::Reader::from_reader(csv_str.as_bytes());
        let mut similarity = HashMap::new();
        let header_hero_names: Vec<Hero> = reader
            .headers()?
            .into_iter()
            .skip(1) // skip "hero" column
            .map(|s| Hero::try_from(s.to_string()))
            .try_collect()?;
        for (i, record) in reader.records().enumerate() {
            let row = record?;
            let mut row_iter = row.into_iter();
            let hero = Hero::try_from(
                row_iter
                    .next()
                    .context(format!("Row {} does not contain hero name.", i))?
                    .to_string(),
            )?;
            let hero_similarities: Vec<f64> = row_iter.map(|s| s.parse::<f64>()).try_collect()?;
            if hero_similarities.len() != header_hero_names.len() {
                bail!("Row {} does not contain all columns.", i);
            }
            let sorted_heroes = header_hero_names
                .iter()
                .cloned()
                .zip(hero_similarities.into_iter())
                .sorted_by(|a, b| f64::total_cmp(&b.1, &a.1))
                .map(|t| t.0)
                .collect_vec();
            if hero != sorted_heroes[0] {
                bail!("Hero is not his own closest hero: {}.", hero);
            }
            similarity.insert(hero, sorted_heroes.into_iter().skip(1).collect());
        }
        Ok(similarity)
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
            info!("Fallback to random hero assignment.");
            return self.assign_random_hero(hero_pool);
        }
        heroes.into_iter().choose(&mut rand::thread_rng())
    }

    fn assign_similar_hero(
        &mut self,
        hero: &Hero,
        hero_pool: &HashSet<Hero>,
        tag: &HeroTag,
    ) -> Option<Hero> {
        if let Some(similar_heroes) = self.hero_similarity.get(hero) {
            let mut sampled_similar_heroes = Vec::new();
            for similar_hero in similar_heroes.iter() {
                if hero_pool.contains(similar_hero) && !self.taken.contains(similar_hero) {
                    sampled_similar_heroes.push(similar_hero.clone());
                    if sampled_similar_heroes.len() == 3 {
                        break;
                    }
                }
            }
            if !sampled_similar_heroes.is_empty() {
                return sampled_similar_heroes
                    .into_iter()
                    .choose(&mut rand::thread_rng());
            }
        }
        info!("Fallback to tagged hero assignment.");
        self.assign_tagged_hero(hero_pool, tag)
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
                let mut paired_hero: Option<Hero> = None;
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
                        .is_some_and(|x| x == true)
                    {
                        continue;
                    }

                    let assigned_pick = if paired_hero.is_some() {
                        self.assign_similar_hero(&paired_hero.clone().unwrap(), hero_pool, pair_tag)
                    } else {
                        paired_hero = self.assign_tagged_hero(hero_pool, pair_tag);
                        paired_hero.clone()
                    };
                    if let Some(pick) = assigned_pick {
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
