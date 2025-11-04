#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use eloelo_model::player::DiscordUsername;
    use itertools::Itertools;

    use crate::eloelo::elodisco::{
        dota_bot::Hero,
        hero_assignment_strategy::{
            DotaTeam, HeroAssignmentStrategy, PlayerInfo, RandomHeroPool, TaggedHeroPool,
        },
    };

    const N: usize = 100;

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
                dota_team: DotaTeam::Radiant,
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
                dota_team: DotaTeam::Dire,
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
                dota_team: DotaTeam::Dire,
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
                dota_team: DotaTeam::Radiant,
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
                assert_eq!(player.number_of_heroes_shown as usize, assignement.len());
                assert!(assignment_in(
                    &assignement,
                    &default_hero_pool().into_iter().collect()
                ));
            }
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
        let mut tag_assign = TaggedHeroPool::new();
        for _ in 0..N {
            let players: Vec<(PlayerInfo, Vec<Hero>)> = default_players(default_hero_pool());
            tag_assign.clear();
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
        let mut tag_assign = TaggedHeroPool::new();
        for _ in 0..N {
            let players = default_players(small_hero_pool());
            tag_assign.clear();
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
        let mut tag_assign = TaggedHeroPool::new();
        for _ in 0..N {
            let players = default_players(default_hero_pool());
            tag_assign.clear();
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
        let mut tag_assign = TaggedHeroPool::new();
        for _ in 0..N {
            let players = default_players(default_hero_pool());
            tag_assign.clear();
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
                assert_ne!(p1.dota_team, p2.dota_team);
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
            let reroll = random_assign
                .reroll(&players[0].0.name, players[0].1.clone())
                .unwrap();
            assert!(no_overlaps_between(
                &players_assignement[&players[0].0],
                &reroll
            ));
        }
    }

    #[test]
    fn test_reroll_tags() {
        let mut tag_assign = TaggedHeroPool::new();
        for _ in 0..N {
            let players = few_players(default_hero_pool());
            tag_assign.clear();
            let players_assignement = tag_assign.assign_heroes(players.clone());
            let reroll = tag_assign
                .reroll(&players[0].0.name, players[0].1.clone())
                .unwrap();
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
