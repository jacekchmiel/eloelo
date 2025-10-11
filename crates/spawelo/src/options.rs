use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct SpaweloOptions {
    pub pity_bouns_factor: f32,
    pub pity_bonus_min_loses: i32,
}

impl Default for SpaweloOptions {
    fn default() -> Self {
        Self {
            pity_bouns_factor: 0.5,
            pity_bonus_min_loses: 2,
        }
    }
}
