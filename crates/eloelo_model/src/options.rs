use crate::decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum OptionValue {
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    Text(String),
    /// One of `DescribedOption::choices`, by its value.
    Choice(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionChoice {
    /// Serialized value, sent back by the UI.
    pub value: String,
    /// Name shown in the UI.
    pub name: String,
}

/// An enum shown in the options form as a choice between its values. The option's value
/// round-trips through the enum's serde representation, which must be a string.
pub trait ChoiceOption: Serialize + Sized {
    /// All values, each with the name shown in the UI.
    fn choices() -> Vec<(Self, &'static str)>;
}

fn choice_value(value: &impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        other => panic!("Choice option value must serialize to a string, got {other:?}"),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribedOption {
    /// Key used to construct response from UI. Must be a camelCase string matching corresponding option field name.
    pub key: String,
    pub name: String,
    #[serde(flatten)]
    pub value: OptionValue,
    /// Values a `Choice` option can take.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<OptionChoice>,
}

impl DescribedOption {
    #[allow(dead_code)]
    pub fn with_bool(value: impl Into<bool>, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Boolean(value.into()),
            choices: Vec::new(),
        }
    }

    pub fn with_int(value: impl Into<i64>, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Integer(value.into()),
            choices: Vec::new(),
        }
    }

    pub fn with_decimal(value: Decimal, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Decimal(value),
            choices: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_text(value: impl ToString, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Text(value.to_string()),
            choices: Vec::new(),
        }
    }

    pub fn with_choice<T: ChoiceOption>(value: &T, key: &str, name: &str) -> DescribedOption {
        DescribedOption {
            key: String::from(key),
            name: String::from(name),
            value: OptionValue::Choice(choice_value(value)),
            choices: T::choices()
                .into_iter()
                .map(|(value, name)| OptionChoice {
                    value: choice_value(&value),
                    name: String::from(name),
                })
                .collect(),
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
                choices: Vec::new(),
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

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    enum Fruit {
        Apple,
        BlueBerry,
    }

    impl ChoiceOption for Fruit {
        fn choices() -> Vec<(Self, &'static str)> {
            vec![(Fruit::Apple, "Apple"), (Fruit::BlueBerry, "Blue berry")]
        }
    }

    #[test]
    fn serialize_choice_option() -> Result<()> {
        let option = DescribedOption::with_choice(&Fruit::BlueBerry, "fruit", "Fruit");
        let expected_json: Value = serde_json::from_str(
            r#"{
                "key": "fruit",
                "name": "Fruit",
                "type": "choice",
                "value": "blueBerry",
                "choices": [
                    { "value": "apple", "name": "Apple" },
                    { "value": "blueBerry", "name": "Blue berry" }
                ]
            }"#,
        )?;
        assert_eq!(serde_json::to_value(option)?, expected_json);
        // The UI sends the value back, which must deserialize into the enum.
        assert_eq!(
            serde_json::from_str::<Fruit>(r#""blueBerry""#)?,
            Fruit::BlueBerry
        );
        Ok(())
    }
}
