use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decimal(String);

impl Decimal {
    /// Panics when literal does not represent a correct decimal.
    pub fn new(literal: &str) -> Self {
        Decimal(literal.into())
            .invariant_ensured()
            .expect("Invalid decimal literal")
    }

    fn invariant_ensured(self) -> Option<Self> {
        let v = self.as_f64();
        if v.is_nan() || v.is_infinite() {
            None
        } else {
            Some(self)
        }
    }

    pub fn as_f64(&self) -> f64 {
        self.0.parse().expect("invalid decimal")
    }

    pub fn as_f32(&self) -> f32 {
        self.0.parse().expect("invalid decimal")
    }

    pub fn with_precision(value: f64, precision: usize) -> Self {
        Decimal(format!("{value:.0$}", precision))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn truncate_to_precision(&self, precision: usize) -> Self {
        // Likely this is prone to numerical errors but don't care until problematic.
        Decimal::with_precision(self.as_f64(), precision)
    }
}

impl Into<f64> for Decimal {
    fn into(self) -> f64 {
        self.as_f64()
    }
}

impl Into<f32> for Decimal {
    fn into(self) -> f32 {
        self.as_f32()
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.as_f64() == other.as_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn with_precision() {
        assert_eq!(Decimal::with_precision(1.11111111111, 1).as_str(), "1.1");
        assert_eq!(Decimal::with_precision(1.11111111111, 2).as_str(), "1.11");
        assert_eq!(Decimal::with_precision(1.11111111111, 3).as_str(), "1.111");
    }

    #[test]
    fn serialize() -> Result<()> {
        assert_eq!(
            serde_json::to_value(Decimal::with_precision(37.21, 2))?,
            serde_json::Value::String("37.21".into())
        );
        Ok(())
    }

    #[test]
    fn deserialize() -> Result<()> {
        assert_eq!(
            serde_json::from_str::<Decimal>("\"21.37\"")?,
            Decimal::new("21.37")
        );
        Ok(())
    }

    #[test]
    fn eq() -> Result<()> {
        assert_eq!(Decimal::new("0.1"), Decimal::new("0.1"));
        assert_eq!(Decimal::new("0.1"), Decimal::new("0.10"));
        assert_ne!(Decimal::new("0.1"), Decimal::new("0.11"));
        Ok(())
    }

    #[test]
    #[should_panic]
    fn nan() {
        let _d = Decimal::new("NaN");
    }

    #[test]
    #[should_panic]
    fn inf() {
        let _d = Decimal::new("+inf");
    }

    #[test]
    #[should_panic]
    fn invalid_str() {
        let _d = Decimal::new("whatever");
    }
}
