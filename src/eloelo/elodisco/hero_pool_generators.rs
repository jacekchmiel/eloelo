use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    convert::identity,
    fmt::Display,
    iter::zip,
};

use anyhow::{format_err, Error, Result};
use eloelo_model::player::DiscordUsername;
use itertools::Itertools;
use rand::seq::{IteratorRandom, SliceRandom};

use crate::eloelo::elodisco::dota_bot::Hero;

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct PlayerInfo {
    pub name: DiscordUsername,
    pub elo: i32,
    pub is_radiant: bool,
    pub number_of_heroes_shown: u32,
}

pub trait HeroPoolGenerator {
    fn assign_heroes(
        &mut self,
        hero_pools: Vec<(PlayerInfo, Vec<Hero>)>,
    ) -> HashMap<PlayerInfo, Vec<Hero>>;

    fn reroll_user_heroes(&mut self, player: &DiscordUsername, hero_pool: Vec<Hero>) -> Vec<Hero>;

    fn clear(&mut self);
}

#[derive(Default)]
pub struct RandomHeroPool {
    pub hero_assignement: HashMap<PlayerInfo, Vec<Hero>>,
    pub taken: HashSet<Hero>,
}

impl HeroPoolGenerator for RandomHeroPool {
    fn assign_heroes(
        &mut self,
        mut hero_pools: Vec<(PlayerInfo, Vec<Hero>)>,
    ) -> HashMap<PlayerInfo, Vec<Hero>> {
        // Sort players by pool length
        hero_pools.sort_by_key(|v| (v.1.len(), rand::random::<i32>())); // TODO: randomize order of same-size users
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
                    .and_then(|v| Some(v.len() == (player.number_of_heroes_shown as usize)))
                    .is_some_and(identity)
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

    fn reroll_user_heroes(
        &mut self,
        username: &DiscordUsername,
        hero_pool: Vec<Hero>,
    ) -> Vec<Hero> {
        if self.hero_assignement.is_empty() {
            return Vec::new();
        }
        let player = if let Some(p) = self.hero_assignement.keys().find(|p| p.name == *username) {
            p.clone()
        } else {
            return Vec::new();
        };
        let old_picks = self.hero_assignement.get(&player).unwrap().clone();
        self.hero_assignement.get_mut(&player).unwrap().clear();
        let mut assignement = self.assign_heroes(vec![(player.clone(), hero_pool)]);
        let new_picks = assignement.entry(player.clone()).or_default().to_vec();
        self.taken = self
            .taken
            .union(&new_picks.iter().cloned().collect())
            .cloned()
            .collect::<HashSet<_>>()
            .difference(&old_picks.iter().cloned().collect())
            .cloned()
            .collect();
        self.hero_assignement
            .entry(player.clone())
            .insert_entry(new_picks.clone());
        new_picks
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeroTag(String);

impl HeroTag {
    pub fn all() -> HashSet<HeroTag> {
        vec![
            HeroTag("Carry".to_string()),
            HeroTag("Core".to_string()),
            HeroTag("Support".to_string()),
        ]
        .into_iter()
        .collect()
    }

    pub fn next_tag(tag_id: usize) -> HeroTag {
        vec![
            HeroTag("Core".to_string()),
            HeroTag("Support".to_string()),
            HeroTag("Carry".to_string()),
        ][tag_id % 3]
            .clone()
    }
}

impl TryFrom<String> for HeroTag {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if HeroTag::all().contains(value.as_str()) {
            Ok(HeroTag(value))
        } else {
            Err(format_err!(
                "Incorrect hero name: \"{}\". See `all` for list of valid names.",
                value
            ))
        }
    }
}

#[cfg(test)]
impl From<&str> for HeroTag {
    fn from(value: &str) -> Self {
        HeroTag::try_from(String::from(value)).unwrap()
    }
}

impl Display for HeroTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Borrow<str> for HeroTag {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<HeroTag> for HeroTag {
    fn as_ref(&self) -> &HeroTag {
        &self
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
        include_str!("dota_heroes.csv")
            .split("\n")
            .map(|s| {
                let raw: Vec<&str> = s.split(",").map(|s| s.trim()).collect();
                (raw[0].to_string(), raw[1].to_string())
            })
            .fold(HashMap::new(), |mut m, (hero_name, tag_name)| {
                if let Some(hero) = Hero::try_from(hero_name).ok() {
                    if let Some(tag) = HeroTag::try_from(tag_name).ok() {
                        m.entry(tag).or_default().insert(hero);
                    }
                }
                m
            })
    }

    fn deduce_tag(&self, hero: &Hero) -> HeroTag {
        for (tag, heroes) in self.tags.iter() {
            if heroes.contains(hero) {
                return tag.clone();
            }
        }
        HeroTag("Support".to_string())
    }

    fn assign_random_hero(&mut self, hero_pool: &HashSet<Hero>) -> Option<Hero> {
        hero_pool
            .difference(&self.taken)
            .cloned()
            .choose(&mut rand::thread_rng())
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

impl HeroPoolGenerator for TaggedHeroPool {
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
            .filter(|(p, _)| p.is_radiant)
            .sorted_by_key(|(p, _)| -p.elo)
            .collect_vec();
        let dire = hero_pools_sets
            .iter()
            .filter(|(p, _)| !p.is_radiant)
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
                    let (player, hero_pool) = team.get(*pair_id).unwrap();
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

    fn reroll_user_heroes(
        &mut self,
        username: &DiscordUsername,
        hero_pool: Vec<Hero>,
    ) -> Vec<Hero> {
        if self.hero_assignement.is_empty() {
            return Vec::new();
        }
        let player = if let Some(p) = self.hero_assignement.keys().find(|p| p.name == *username) {
            p.clone()
        } else {
            return Vec::new();
        };
        let player_tag =
            self.deduce_tag(&self.hero_assignement.get(&player).unwrap().get(0).unwrap());
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
            .cloned()
            .collect::<HashSet<_>>()
            .difference(&old_picks.iter().cloned().collect())
            .cloned()
            .collect();
        self.hero_assignement
            .entry(player)
            .insert_entry(new_picks.clone());
        new_picks
    }

    fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use eloelo_model::player::DiscordUsername;
    use itertools::Itertools;

    use crate::eloelo::elodisco::{
        dota_bot::Hero,
        hero_pool_generators::{HeroPoolGenerator, PlayerInfo, RandomHeroPool, TaggedHeroPool},
    };

    const N: usize = 10;

    fn default_hero_pool() -> Vec<Hero> {
        vec![
            Hero::from("Puck"),
            Hero::from("Pudge"),
            Hero::from("Razor"),
            Hero::from("Io"),
            Hero::from("Lion"),
            Hero::from("Lich"),
        ]
    }

    fn small_hero_pool() -> Vec<Hero> {
        vec![
            Hero::from("Puck"),
            Hero::from("Pudge"),
            Hero::from("Io"),
            Hero::from("Lion"),
        ]
    }

    fn player_j(pool: &Vec<Hero>) -> (PlayerInfo, Vec<Hero>) {
        (
            PlayerInfo {
                name: DiscordUsername::from("j".to_string()),
                elo: 1000,
                is_radiant: true,
                number_of_heroes_shown: 2,
            },
            pool.clone(),
        )
    }

    fn player_bixkog(pool: &Vec<Hero>) -> (PlayerInfo, Vec<Hero>) {
        (
            PlayerInfo {
                name: DiscordUsername::from("bixkog".to_string()),
                elo: 1000,
                is_radiant: false,
                number_of_heroes_shown: 1,
            },
            pool.clone(),
        )
    }

    fn player_dragon(pool: &Vec<Hero>) -> (PlayerInfo, Vec<Hero>) {
        (
            PlayerInfo {
                name: DiscordUsername::from("dragon".to_string()),
                elo: 100,
                is_radiant: false,
                number_of_heroes_shown: 2,
            },
            pool.clone(),
        )
    }

    fn player_goovie(pool: &Vec<Hero>) -> (PlayerInfo, Vec<Hero>) {
        (
            PlayerInfo {
                name: DiscordUsername::from("goovie".to_string()),
                elo: 100,
                is_radiant: true,
                number_of_heroes_shown: 1,
            },
            pool.clone(),
        )
    }

    fn default_players(pool: Vec<Hero>) -> Vec<(PlayerInfo, Vec<Hero>)> {
        vec![
            player_j(&pool),
            player_bixkog(&pool),
            player_dragon(&pool),
            player_goovie(&pool),
        ]
    }

    fn few_players(pool: Vec<Hero>) -> Vec<(PlayerInfo, Vec<Hero>)> {
        vec![player_bixkog(&pool), player_goovie(&pool)]
    }

    fn no_duplicates(assignments: &HashMap<PlayerInfo, Vec<Hero>>) -> bool {
        let total_len = assignments.values().flatten().count();
        let unique_len = assignments.values().flatten().unique().count();
        total_len == unique_len
    }

    fn assignment_in(assignment: &Vec<Hero>, set: &HashSet<Hero>) -> bool {
        set.is_superset(&assignment.iter().cloned().collect())
    }

    // every HeroPoolGenerator should pass this test
    #[test]
    fn test_base_random() {
        for _ in 0..N {
            let players = default_players(default_hero_pool());
            let mut random_assign = RandomHeroPool::default();
            let players_assignement = random_assign.assign_heroes(players);
            assert!(no_duplicates(&players_assignement));
            for (player, assignement) in players_assignement {
                println!("{}: {:?}", player.name, assignement);
                assert_eq!(player.number_of_heroes_shown as usize, assignement.len());
                assert!(assignment_in(
                    &assignement,
                    &default_hero_pool().into_iter().collect()
                ));
            }
            println!("{:?}", random_assign.taken);
        }
    }

    // every HeroPoolGenerator should pass this test
    #[test]
    fn test_small_pool_random() {
        for _ in 0..N {
            let players = default_players(small_hero_pool());
            let mut random_assign = RandomHeroPool::default();
            let players_assignement = random_assign.assign_heroes(players);
            assert!(no_duplicates(&players_assignement));
            for (_, assignement) in players_assignement {
                assert_eq!(assignement.len(), 1);
            }
        }
    }

    // every HeroPoolGenerator should pass this test
    #[test]
    fn test_randomness_random() {
        let mut all_assignements = Vec::new();
        let mut random_assign = RandomHeroPool::default();
        for _ in 0..N {
            let players = few_players(default_hero_pool());
            random_assign.clear();
            let players_assignement = random_assign.assign_heroes(players);
            all_assignements.push(players_assignement);
        }
        assert!(
            all_assignements
                .into_iter()
                .map(|hm| hm.into_iter().sorted().map(|(_, v)| v).collect_vec())
                .unique()
                .count()
                > 1
        );
    }

    #[test]
    fn test_base_tags() {
        for _ in 0..N {
            let players: Vec<(PlayerInfo, Vec<Hero>)> = default_players(default_hero_pool());
            let mut tag_assign = TaggedHeroPool::new();
            let players_assignement = tag_assign.assign_heroes(players);
            assert!(no_duplicates(&players_assignement));
            for (player, assignement) in players_assignement.iter() {
                assert_eq!(player.number_of_heroes_shown as usize, assignement.len());
                assert!(assignment_in(
                    assignement,
                    &default_hero_pool().into_iter().collect()
                ));
            }
        }
    }

    #[test]
    fn test_small_pool_tags() {
        for _ in 0..N {
            let players = default_players(small_hero_pool());
            let mut tag_assign = TaggedHeroPool::new();
            let players_assignement = tag_assign.assign_heroes(players);
            assert!(no_duplicates(&players_assignement));
            for (_, assignement) in players_assignement {
                assert_eq!(assignement.len(), 1);
            }
        }
    }

    #[test]
    fn test_randomness_tags() {
        let mut all_assignements = Vec::new();
        let mut tag_assign = TaggedHeroPool::new();
        for _ in 0..N {
            let players = few_players(default_hero_pool());
            tag_assign.clear();
            let players_assignement = tag_assign.assign_heroes(players);
            all_assignements.push(players_assignement);
        }
        assert!(
            all_assignements
                .into_iter()
                .map(|hm| hm.into_iter().sorted().map(|(_, v)| v).collect_vec())
                .unique()
                .count()
                > 1
        );
    }

    #[test]
    fn test_tags_consistent() {
        for _ in 0..N {
            let players = default_players(default_hero_pool());
            let mut tag_assign = TaggedHeroPool::new();
            let players_assignement = tag_assign.assign_heroes(players);
            for (_, assignement) in players_assignement.iter() {
                assert_eq!(
                    assignement
                        .iter()
                        .map(|h| tag_assign.deduce_tag(h))
                        .unique()
                        .count(),
                    1
                );
            }
        }
    }

    #[test]
    fn test_tags_correspondence() {
        for _ in 0..N {
            let players = default_players(default_hero_pool());
            let mut tag_assign = TaggedHeroPool::new();
            let players_assignement = tag_assign.assign_heroes(players);
            for mut pair in players_assignement
                .into_iter()
                .sorted_by_key(|(p, _)| p.elo)
                .chunks(2)
                .into_iter()
            {
                let (p1, assignement1) = pair.next().unwrap();
                let (p2, assignement2) = pair.next().unwrap();
                assert_eq!(p1.elo, p2.elo);
                assert_ne!(p1.is_radiant, p2.is_radiant);
                assert_eq!(
                    assignement1
                        .iter()
                        .map(|h| tag_assign.deduce_tag(h))
                        .next()
                        .unwrap(),
                    assignement2
                        .iter()
                        .map(|h| tag_assign.deduce_tag(h))
                        .next()
                        .unwrap()
                );
            }
        }
    }

    fn no_overlaps_between(lhs: &Vec<Hero>, rhs: &Vec<Hero>) -> bool {
        !(lhs.iter().any(|h| rhs.contains(h)) || rhs.iter().any(|h| lhs.contains(h)))
    }

    // every HeroPoolGenerator should pass this test
    #[test]
    fn test_reroll_random() {
        for _ in 0..N {
            let players = few_players(small_hero_pool());
            let mut random_assign = RandomHeroPool::default();
            let players_assignement = random_assign.assign_heroes(players.clone());
            let reroll = random_assign.reroll_user_heroes(&players[0].0.name, players[0].1.clone());
            assert!(no_overlaps_between(
                &players_assignement[&players[0].0],
                &reroll
            ));
        }
    }

    #[test]
    fn test_reroll_tags() {
        for _ in 0..N {
            let players = few_players(default_hero_pool());
            let mut tag_assign = TaggedHeroPool::new();
            let players_assignement = tag_assign.assign_heroes(players.clone());
            let reroll = tag_assign.reroll_user_heroes(&players[0].0.name, players[0].1.clone());
            assert!(no_overlaps_between(
                &players_assignement[&players[0].0],
                &reroll
            ));
            assert_eq!(
                reroll
                    .iter()
                    .map(|h| tag_assign.deduce_tag(h))
                    .unique()
                    .count(),
                1
            );
            assert_eq!(
                players_assignement[&players[0].0]
                    .iter()
                    .map(|h| tag_assign.deduce_tag(h))
                    .next()
                    .unwrap(),
                reroll
                    .iter()
                    .map(|h| tag_assign.deduce_tag(h))
                    .next()
                    .unwrap()
            );
        }
    }
}
