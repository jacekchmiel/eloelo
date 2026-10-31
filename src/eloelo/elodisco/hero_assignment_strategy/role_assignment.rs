use itertools::Itertools;
use rand::{seq::SliceRandom, thread_rng, Rng};

use super::HeroTag;

/// How roles are dealt to the players of a match.
///
/// Both teams are sorted by ELO. A position is a rank in that order: the players of both teams at
/// the same rank form a pair and get the same role. The bigger team's lowest-ELO players have
/// positions without a pair.
#[derive(Debug, Clone, PartialEq)]
pub enum RoleAssignment {
    OrderedRotation,
}

impl RoleAssignment {
    /// The role of every position, in the order heroes are assigned to the positions.
    pub fn assign(&self, t1_len: usize, t2_len: usize) -> Vec<(usize, HeroTag)> {
        match self {
            RoleAssignment::OrderedRotation => ordered_rotation(t1_len, t2_len),
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

fn random_tag() -> HeroTag {
    match thread_rng().gen_range(0..3) {
        0 => HeroTag::Carry,
        1 => HeroTag::Core,
        _ => HeroTag::Support,
    }
}
