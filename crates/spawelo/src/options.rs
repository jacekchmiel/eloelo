use eloelo_model::decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SpaweloOptions {
    pub pity_bonus_additive: bool,
    pub pity_bonus_multiplicative: bool,
    pub pity_bonus_factor: Decimal,
    pub pity_bonus_min_loses: i32,
    pub pity_bonus_additive_amount: i32,
}

impl Default for SpaweloOptions {
    fn default() -> Self {
        Self {
            pity_bonus_additive: true,
            pity_bonus_multiplicative: false,
            pity_bonus_factor: Decimal::with_precision(0.98, 2),
            pity_bonus_min_loses: 2,
            pity_bonus_additive_amount: 100,
        }
    }
}
