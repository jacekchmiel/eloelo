use eloelo_model::options::{DescribedOption, DescribedOptionsGroup, Options};
use serde::{Deserialize, Serialize};
use spawelo::{MlEloOptions, PityBonusOptions, SpaweloOptions};

use crate::eloelo::elodisco::hero_assignment_strategy::HeroAssignmentStrategyKind;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralOptions {
    pub enable_autogrzybke: bool,
}

impl Default for GeneralOptions {
    fn default() -> Self {
        Self {
            enable_autogrzybke: true,
        }
    }
}

impl Options for GeneralOptions {
    fn key() -> String {
        String::from("general")
    }

    fn name() -> String {
        String::from("General")
    }

    fn to_described_options(&self) -> Vec<DescribedOption> {
        vec![DescribedOption::with_bool(
            self.enable_autogrzybke,
            "enableAutogrzybke",
            "Enable autogrzybke",
        )]
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EloEloOptions {
    pub general: GeneralOptions,
    pub spawelo: SpaweloOptions,
    #[serde(default)]
    pub hero_assignment: HeroAssignmentOptions,
}

impl EloEloOptions {
    pub fn to_described_options_group_vec(&self) -> Vec<DescribedOptionsGroup> {
        vec![
            self.general.to_described_options_group(),
            self.spawelo.ml_elo.to_described_options_group(),
            self.spawelo.pity_bonus.to_described_options_group(),
            self.hero_assignment.to_described_options_group(),
        ]
    }
}

/// How the Discord bot assigns heroes. Sent to the bot with every `MatchStart`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct HeroAssignmentOptions {
    pub strategy: HeroAssignmentStrategyKind,
}

impl Options for HeroAssignmentOptions {
    fn key() -> String {
        String::from("heroAssignment")
    }

    fn name() -> String {
        String::from("Hero assignment")
    }

    fn to_described_options(&self) -> Vec<DescribedOption> {
        vec![DescribedOption::with_choice(
            &self.strategy,
            "strategy",
            "Strategy",
        )]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct EloEloOptionsTransport {
    pub ml_elo: MlEloOptions,
    pub pity_bonus: PityBonusOptions,
    pub general: GeneralOptions,
    pub hero_assignment: HeroAssignmentOptions,
}

impl Into<EloEloOptions> for EloEloOptionsTransport {
    fn into(self) -> EloEloOptions {
        EloEloOptions {
            general: self.general,
            spawelo: SpaweloOptions {
                ml_elo: self.ml_elo,
                pity_bonus: self.pity_bonus,
            },
            hero_assignment: self.hero_assignment,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_without_hero_assignment_load_with_default() {
        let yaml = serde_yaml::to_string(&EloEloOptions::default()).unwrap();
        let mut value: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        value
            .as_mapping_mut()
            .unwrap()
            .remove("heroAssignment")
            .unwrap();
        let options: EloEloOptions = serde_yaml::from_value(value).unwrap();
        assert_eq!(options.hero_assignment, HeroAssignmentOptions::default());
    }
}
