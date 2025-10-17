use eloelo_model::decimal::Decimal;
use serde::{Deserialize, Serialize};
use spawelo::SpaweloOptions;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum OptionValue {
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    Text(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribedOption {
    /// Key used to construct response from UI. Must be a camelCase string matching corresponding option field name.
    pub key: String,
    pub name: String,
    #[serde(flatten)]
    pub value: OptionValue,
}

impl DescribedOption {
    #[allow(dead_code)]
    pub fn with_bool(value: impl Into<bool>, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Boolean(value.into()),
        }
    }

    pub fn with_int(value: impl Into<i64>, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Integer(value.into()),
        }
    }

    pub fn with_decimal(value: Decimal, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Decimal(value),
        }
    }

    #[allow(dead_code)]
    pub fn with_text(value: impl ToString, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Text(value.to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribedOptionsGroup {
    pub name: String,
    pub key: String,
    pub options: Vec<DescribedOption>,
}

pub trait Options: Default {
    fn key() -> String;
    fn name() -> String;
    fn to_described_options(&self) -> Vec<DescribedOption>;
    // fn from_described_options(options: &[DescribedOption]) -> Self;

    fn to_described_options_group(&self) -> DescribedOptionsGroup {
        DescribedOptionsGroup {
            name: Self::name(),
            key: Self::key(),
            options: self.to_described_options(),
        }
    }
}

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
    use serde_json::Value;

    #[test]
    fn serialize_described_options_group() -> Result<()> {
        let options = DescribedOptionsGroup {
            name: "Test Options".into(),
            key: "test".into(),
            options: vec![DescribedOption {
                key: "opt1".into(),
                name: "Option 1".into(),
                value: OptionValue::Boolean(true),
            }],
        };
        let options_json = serde_json::to_value(options)?;
        let expected_json: Value = serde_json::from_str(
            r#"{
                "name": "Test Options",
                "key": "test",
                "options": [
                    {
                        "key": "opt1",
                        "name": "Option 1",
                        "type": "boolean",
                        "value": true
                    }
                ]
            }"#,
        )?;
        assert_eq!(options_json, expected_json);
        Ok(())
    }

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
