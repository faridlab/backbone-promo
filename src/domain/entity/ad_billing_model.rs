use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "ad_billing_model", rename_all = "snake_case")]
pub enum AdBillingModel {
    Cpm,
    Cpc,
    Cpa,
    FlatRate,
}

impl std::fmt::Display for AdBillingModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpm => write!(f, "cpm"),
            Self::Cpc => write!(f, "cpc"),
            Self::Cpa => write!(f, "cpa"),
            Self::FlatRate => write!(f, "flat_rate"),
        }
    }
}

impl FromStr for AdBillingModel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cpm" => Ok(Self::Cpm),
            "cpc" => Ok(Self::Cpc),
            "cpa" => Ok(Self::Cpa),
            "flat_rate" => Ok(Self::FlatRate),
            _ => Err(format!("Unknown AdBillingModel variant: {}", s)),
        }
    }
}

impl Default for AdBillingModel {
    fn default() -> Self {
        Self::Cpm
    }
}
