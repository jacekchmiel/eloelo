use eloelo_model::options::{DescribedOption, DescribedOptionsGroup, Options};
use serde::{Deserialize, Serialize};
use spawelo::SpaweloOptions;

#[derive(Clone, Debug, Serialize, Deserialize)]
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
}

impl EloEloOptions {
    pub fn to_described_options_group_vec(&self) -> Vec<DescribedOptionsGroup> {
        vec![
            self.general.to_described_options_group(),
            self.spawelo.to_described_options_group(),
        ]
    }
}
