use eloelo_model::{
    decimal::Decimal,
    options::{DescribedOption, Options},
};
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
    pub lose_streak_max_days: i32,
}

impl Default for SpaweloOptions {
    fn default() -> Self {
        Self {
            pity_bonus_additive: true,
            pity_bonus_multiplicative: false,
            pity_bonus_factor: Decimal::with_precision(-0.02, 2),
            pity_bonus_min_loses: 2,
            pity_bonus_additive_amount: 100,
            lose_streak_max_days: 30,
        }
    }
}

impl Options for SpaweloOptions {
    fn key() -> String {
        "spawelo".into()
    }

    fn name() -> String {
        "Spawelo Options".into()
    }

    fn to_described_options(&self) -> Vec<DescribedOption> {
        vec![
            DescribedOption::with_int(
                self.lose_streak_max_days,
                "loseStreakMaxDays",
                "Lose Streak Max Days",
            ),
            DescribedOption::with_int(
                self.pity_bonus_min_loses,
                "pityBonusMinLoses",
                "Pity Bonus Min Loses",
            ),
            DescribedOption::with_bool(
                self.pity_bonus_additive,
                "pityBonusAdditive",
                "Pity Bonus Additive",
            ),
            DescribedOption::with_int(
                self.pity_bonus_additive_amount,
                "pityBonusAdditiveAmount",
                "Pity Bonus Additive Amount",
            ),
            DescribedOption::with_bool(
                self.pity_bonus_multiplicative,
                "pityBonusMultiplicative",
                "Pity Bonus Multiplicative",
            ),
            DescribedOption::with_decimal(
                self.pity_bonus_factor.clone(),
                "pityBonusFactor",
                "Pity Bonus Factor",
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn deserialize_spawelo_options() -> Result<()> {
        let json_str = r#"{
            "pityBonusFactor": "0.5",
            "pityBonusMinLoses": 5
        }"#;
        assert_eq!(
            serde_json::from_str::<SpaweloOptions>(json_str)?,
            SpaweloOptions {
                pity_bonus_factor: Decimal::new("0.5"),
                pity_bonus_min_loses: 5,
                ..Default::default()
            }
        );
        Ok(())
    }

    #[test]
    fn deserialize_partial_spawelo_options() -> Result<()> {
        let json_str = r#"{
            "pityBonusFactor": "0.5"
        }"#;
        assert_eq!(
            serde_json::from_str::<SpaweloOptions>(json_str)?,
            SpaweloOptions {
                pity_bonus_factor: Decimal::new("0.5"),
                ..Default::default()
            }
        );
        Ok(())
    }
}
