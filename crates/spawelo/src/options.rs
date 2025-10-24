use eloelo_model::{
    decimal::Decimal,
    options::{DescribedOption, Options},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct PityBonusOptions {
    pub additive: bool,
    pub multiplicative: bool,
    pub factor: Decimal,
    pub min_loses: i32,
    pub additive_amount: i32,
    pub lose_streak_max_days: i32,
}

impl Default for PityBonusOptions {
    fn default() -> Self {
        Self {
            additive: true,
            multiplicative: false,
            factor: Decimal::with_precision(-0.02, 2),
            min_loses: 2,
            additive_amount: 100,
            lose_streak_max_days: 30,
        }
    }
}

impl Options for PityBonusOptions {
    fn key() -> String {
        "pityBonus".into()
    }

    fn name() -> String {
        "Pity Bonus Options".into()
    }

    fn to_described_options(&self) -> Vec<DescribedOption> {
        vec![
            DescribedOption::with_int(
                self.lose_streak_max_days,
                "loseStreakMaxDays",
                "Lose Streak Max Age [Days]",
            ),
            DescribedOption::with_int(self.min_loses, "minLoses", "Min Loses"),
            DescribedOption::with_bool(self.additive, "additive", "Additive"),
            DescribedOption::with_int(self.additive_amount, "additiveAmount", "Additive Amount"),
            DescribedOption::with_bool(self.multiplicative, "multiplicative", "Multiplicative"),
            DescribedOption::with_decimal(self.factor.clone(), "factor", "Multiplicative Factor"),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct MlEloOptions {
    pub fake_match_max_days: i32,
    pub max_elo_history: i32,
}

impl Default for MlEloOptions {
    fn default() -> Self {
        Self {
            fake_match_max_days: 99999,
            max_elo_history: 0,
        }
    }
}

impl Options for MlEloOptions {
    fn key() -> String {
        "mlElo".into()
    }

    fn name() -> String {
        "ML ELO Options".into()
    }

    fn to_described_options(&self) -> Vec<DescribedOption> {
        vec![
            DescribedOption::with_int(
                self.fake_match_max_days,
                "fakeMatchMaxDays",
                "Fake Match Max Age [Days]",
            ),
            DescribedOption::with_int(
                self.max_elo_history,
                "maxEloHistory",
                "Max Elo History [Matches]",
            ),
        ]
    }
}
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SpaweloOptions {
    pub ml_elo: MlEloOptions,
    pub pity_bonus: PityBonusOptions,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn deserialize_pity_bonus_options() -> Result<()> {
        let json_str = r#"{
            "factor": "0.5",
            "minLoses": 5
        }"#;
        assert_eq!(
            serde_json::from_str::<PityBonusOptions>(json_str)?,
            PityBonusOptions {
                factor: Decimal::new("0.5"),
                min_loses: 5,
                ..Default::default()
            }
        );
        Ok(())
    }

    #[test]
    fn deserialize_partial_pity_bonus_options() -> Result<()> {
        let json_str = r#"{
            "factor": "0.5"
        }"#;
        assert_eq!(
            serde_json::from_str::<PityBonusOptions>(json_str)?,
            PityBonusOptions {
                factor: Decimal::new("0.5"),
                ..Default::default()
            }
        );
        Ok(())
    }
}
