use eloelo_model::options::ChoiceOption;
use itertools::Itertools;
use log::warn;
use rand::{
    distributions::{Distribution, WeightedIndex},
    seq::SliceRandom,
    thread_rng, Rng,
};
use serde::{Deserialize, Serialize};

use super::HeroTag;

/// How roles are dealt to the players of a match (see `docs/role-assignment.md`).
///
/// Both teams are sorted by ELO. A position is a rank in that order: the players of both teams at
/// the same rank form a pair and get the same role. The bigger team's lowest-ELO players have
/// positions without a pair.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoleAssignment {
    OrderedRotation,
    #[default]
    TeamTable,
}

impl ChoiceOption for RoleAssignment {
    fn choices() -> Vec<(Self, &'static str)> {
        vec![
            (RoleAssignment::OrderedRotation, "Ordered rotation (legacy)"),
            (RoleAssignment::TeamTable, "Team table"),
        ]
    }
}

impl RoleAssignment {
    /// The role of every position, in the order heroes are assigned to the positions.
    pub fn assign(&self, t1_len: usize, t2_len: usize) -> Vec<(usize, HeroTag)> {
        match self {
            RoleAssignment::OrderedRotation => ordered_rotation(t1_len, t2_len),
            RoleAssignment::TeamTable => team_table(t1_len, t2_len),
        }
    }
}

/// Core -> Support -> Carry -> ... dealt to the pairs in a random order. Positions without a pair
/// come last, and the last one gets a random role.
fn ordered_rotation(t1_len: usize, t2_len: usize) -> Vec<(usize, HeroTag)> {
    let (t_min, t_max) = (t1_len.min(t2_len), t1_len.max(t2_len));
    let mut pairing_order = (0..t_min).collect_vec();
    pairing_order.shuffle(&mut thread_rng());
    let mut tags = (0..t_max).map(rotation_tag).collect_vec();
    if t1_len != t2_len {
        // To prevent that last player is always carry, assign random tag to the unpaired players
        tags.last_mut().map(|t| *t = random_tag());
    }
    pairing_order
        .into_iter()
        .chain(t_min..t_max)
        .zip(tags)
        .collect()
}

fn rotation_tag(tag_id: usize) -> HeroTag {
    [HeroTag::Core, HeroTag::Support, HeroTag::Carry][tag_id % 3].clone()
}

/// Numbers of Carry, Core and Support players in a team.
type Composition = [usize; 3];

const ROLES: [HeroTag; 3] = [HeroTag::Carry, HeroTag::Core, HeroTag::Support];

/// Each team gets a composition from `team_compositions`. In an uneven match the extra player gets
/// a role with the odds of any player in a team of the bigger size, and the smaller team a
/// composition that makes the bigger team's a valid one. Roles are dealt to positions at random.
fn team_table(t1_len: usize, t2_len: usize) -> Vec<(usize, HeroTag)> {
    let (t_min, t_max) = (t1_len.min(t2_len), t1_len.max(t2_len));
    let mut rng = thread_rng();
    let (smaller, extra_roles) = match t_max - t_min {
        0 => (
            *team_compositions(t_min).choose(&mut rng).unwrap(),
            Vec::new(),
        ),
        1 => smaller_team_and_extra_role(t_min, t_max)
            .map(|(smaller, role)| (smaller, vec![role]))
            .unwrap_or_else(|| uncovered_team_sizes(t_min, t_max)),
        _ => uncovered_team_sizes(t_min, t_max),
    };
    let mut roles = smaller
        .iter()
        .zip(ROLES)
        .flat_map(|(count, role)| std::iter::repeat(role).take(*count))
        .collect_vec();
    roles.shuffle(&mut rng);
    roles.extend(extra_roles);
    let mut position_tags = roles.into_iter().enumerate().collect_vec();
    position_tags.shuffle(&mut rng);
    position_tags
}

/// The compositions a team may have, equally likely in an even match.
fn team_compositions(size: usize) -> Vec<Composition> {
    match size {
        0 => vec![[0, 0, 0]],
        1 => vec![[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        2 => vec![[1, 0, 1], [0, 1, 1]],
        3 => vec![[1, 1, 1], [0, 2, 1]],
        4 => vec![[1, 2, 1], [1, 1, 2]],
        5 => vec![[1, 2, 2]],
        6 => vec![[2, 2, 2], [1, 3, 2], [1, 2, 3]],
        7 => vec![[2, 3, 2], [2, 2, 3]],
        8 => vec![[3, 3, 2], [3, 2, 3], [2, 3, 3]],
        _ => team_compositions(size - 3)
            .into_iter()
            .map(|c| c.map(|count| count + 1))
            .collect(),
    }
}

/// The compositions a team may have against a team of one more player. 4 against 5 may also go
/// without a Carry: the extra player, the other team's lowest ELO, is then the Carry.
fn smaller_team_compositions(size: usize) -> Vec<Composition> {
    let mut compositions = team_compositions(size);
    if size == 4 {
        compositions.push([0, 2, 2]);
    }
    compositions
}

/// The odds of each role for a player in a team of `size`.
fn role_shares(size: usize) -> [f64; 3] {
    let compositions = team_compositions(size);
    let players = (compositions.len() * size) as f64;
    [0, 1, 2].map(|role| compositions.iter().map(|c| c[role]).sum::<usize>() as f64 / players)
}

/// The extra player's role is drawn with `role_shares` of the bigger team, then the smaller team's
/// composition among those that, with the extra role, make a valid bigger team. Roles that no
/// composition fits are left out.
fn smaller_team_and_extra_role(t_min: usize, t_max: usize) -> Option<(Composition, HeroTag)> {
    let bigger = team_compositions(t_max);
    let shares = role_shares(t_max);
    let mut candidates = Vec::new();
    for role in 0..3 {
        let fitting = smaller_team_compositions(t_min)
            .into_iter()
            .filter(|c| {
                let mut with_extra = *c;
                with_extra[role] += 1;
                bigger.contains(&with_extra)
            })
            .collect_vec();
        let weight = shares[role] / fitting.len() as f64;
        candidates.extend(fitting.into_iter().map(|c| (c, role, weight)));
    }
    let index = WeightedIndex::new(candidates.iter().map(|(_, _, weight)| *weight)).ok()?;
    let (smaller, role, _) = candidates[index.sample(&mut thread_rng())];
    Some((smaller, ROLES[role].clone()))
}

/// Teams differing by more than one player: the smaller team gets a composition from the table and
/// each extra player a role with the odds of the bigger team, without checking the bigger team.
fn uncovered_team_sizes(t_min: usize, t_max: usize) -> (Composition, Vec<HeroTag>) {
    warn!("Team sizes {t_min} and {t_max} aren't covered by the role table.");
    let mut rng = thread_rng();
    let shares = WeightedIndex::new(role_shares(t_max)).unwrap();
    let extra_roles = (t_min..t_max)
        .map(|_| ROLES[shares.sample(&mut rng)].clone())
        .collect();
    (
        *team_compositions(t_min).choose(&mut rng).unwrap(),
        extra_roles,
    )
}

fn random_tag() -> HeroTag {
    match thread_rng().gen_range(0..3) {
        0 => HeroTag::Carry,
        1 => HeroTag::Core,
        _ => HeroTag::Support,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const N: usize = 1000;

    /// The composition of the team of `team_len` players: the positions below `team_len`.
    fn composition(position_tags: &[(usize, HeroTag)], team_len: usize) -> Composition {
        let mut composition = [0; 3];
        for (position, tag) in position_tags {
            if *position < team_len {
                composition[ROLES.iter().position(|r| r == tag).unwrap()] += 1;
            }
        }
        composition
    }

    fn seen<T: std::hash::Hash + Eq>(f: impl Fn() -> T) -> HashSet<T> {
        (0..N).map(|_| f()).collect()
    }

    #[test]
    fn every_position_gets_one_role() {
        for role_assignment in [RoleAssignment::OrderedRotation, RoleAssignment::TeamTable] {
            for (t1_len, t2_len) in [
                (0, 1),
                (1, 1),
                (2, 3),
                (4, 4),
                (5, 4),
                (6, 7),
                (9, 10),
                (3, 5),
            ] {
                let position_tags = role_assignment.assign(t1_len, t2_len);
                let positions = position_tags.iter().map(|(p, _)| *p).sorted().collect_vec();
                assert_eq!(positions, (0..t1_len.max(t2_len)).collect_vec());
            }
        }
    }

    #[test]
    fn ordered_rotation_compositions() {
        for _ in 0..N {
            let roles = RoleAssignment::OrderedRotation;
            assert_eq!(composition(&roles.assign(5, 5), 5), [1, 2, 2]);
            assert_eq!(composition(&roles.assign(6, 6), 6), [2, 2, 2]);
            assert_eq!(composition(&roles.assign(7, 7), 7), [2, 3, 2]);
        }
    }

    #[test]
    fn team_table_even_match_uses_every_composition_of_the_table() {
        for size in 1..=10 {
            let compositions = seen(|| composition(&team_table(size, size), size));
            assert_eq!(
                compositions,
                team_compositions(size).into_iter().collect(),
                "{size}v{size}"
            );
        }
    }

    #[test]
    fn team_table_uneven_match_keeps_both_teams_in_the_table() {
        for t_min in 1..=9 {
            let t_max = t_min + 1;
            for _ in 0..N {
                let position_tags = team_table(t_min, t_max);
                let smaller = composition(&position_tags, t_min);
                let bigger = composition(&position_tags, t_max);
                assert!(
                    smaller_team_compositions(t_min).contains(&smaller),
                    "{t_min}v{t_max}: {smaller:?}"
                );
                assert!(
                    team_compositions(t_max).contains(&bigger),
                    "{t_min}v{t_max}: {bigger:?}"
                );
            }
        }
    }

    #[test]
    fn team_table_extra_player_gets_every_role() {
        for t_min in 3..=8 {
            let extra_roles = seen(|| {
                let position_tags = team_table(t_min, t_min + 1);
                position_tags
                    .into_iter()
                    .find(|(p, _)| *p == t_min)
                    .unwrap()
                    .1
            });
            assert_eq!(extra_roles.len(), 3, "{t_min}v{}", t_min + 1);
        }
    }

    #[test]
    fn team_table_uneven_compositions() {
        // 4v5: the 4-player team is 1/2/2 without any one role.
        let smaller = seen(|| composition(&team_table(4, 5), 4));
        assert_eq!(smaller, HashSet::from([[1, 2, 1], [1, 1, 2], [0, 2, 2]]));
        // 5v6: the 5-player team stays 1/2/2, the 6-player team gets any 6-player composition.
        let smaller = seen(|| composition(&team_table(5, 6), 5));
        assert_eq!(smaller, HashSet::from([[1, 2, 2]]));
        let bigger = seen(|| composition(&team_table(5, 6), 6));
        assert_eq!(bigger, team_compositions(6).into_iter().collect());
    }

    #[test]
    fn team_table_role_shares() {
        assert_eq!(role_shares(5), [0.2, 0.4, 0.4]);
        assert_eq!(role_shares(6), [4.0 / 18.0, 7.0 / 18.0, 7.0 / 18.0]);
    }
}
