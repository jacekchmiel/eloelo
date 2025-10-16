use eloelo_model::decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SpaweloOptions {
    pub pity_bonus_enabled: bool,
    pub pity_bonus_factor: Decimal,
    pub pity_bonus_min_loses: i32,
}

impl Default for SpaweloOptions {
    fn default() -> Self {
        Self {
            pity_bonus_enabled: true,
            pity_bonus_factor: Decimal::with_precision(0.5, 2),
            pity_bonus_min_loses: 2,
        }
    }
}
