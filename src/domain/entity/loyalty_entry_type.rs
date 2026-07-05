use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "loyalty_entry_type", rename_all = "snake_case")]
pub enum LoyaltyEntryType {
    Earned,
    Redeemed,
    Expired,
}

impl std::fmt::Display for LoyaltyEntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Earned => write!(f, "earned"),
            Self::Redeemed => write!(f, "redeemed"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for LoyaltyEntryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "earned" => Ok(Self::Earned),
            "redeemed" => Ok(Self::Redeemed),
            "expired" => Ok(Self::Expired),
            _ => Err(format!("Unknown LoyaltyEntryType variant: {}", s)),
        }
    }
}

impl Default for LoyaltyEntryType {
    fn default() -> Self {
        Self::Earned
    }
}
