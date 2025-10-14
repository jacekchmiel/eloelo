use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SpaweloOptions {
    pub pity_bonus_enabled: bool,
    pub pity_bonus_factor: f32,
    pub pity_bonus_min_loses: i32,
}

impl Default for SpaweloOptions {
    fn default() -> Self {
        Self {
            pity_bonus_enabled: true,
            pity_bonus_factor: 0.5,
            pity_bonus_min_loses: 2,
        }
    }
}
