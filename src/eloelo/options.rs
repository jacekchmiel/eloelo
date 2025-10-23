use eloelo_model::options::{DescribedOptionsGroup, Options as _};
use serde::{Deserialize, Serialize};
use spawelo::SpaweloOptions;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EloEloOptions {
    pub spawelo: SpaweloOptions,
}

impl EloEloOptions {
    pub fn to_described_options_group_vec(&self) -> Vec<DescribedOptionsGroup> {
        vec![self.spawelo.to_described_options_group()]
    }
}
