use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "loyalty_program_type", rename_all = "snake_case")]
pub enum LoyaltyProgramType {
    SingleTier,
    MultipleTier,
}

impl std::fmt::Display for LoyaltyProgramType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SingleTier => write!(f, "single_tier"),
            Self::MultipleTier => write!(f, "multiple_tier"),
        }
    }
}

impl FromStr for LoyaltyProgramType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "single_tier" => Ok(Self::SingleTier),
            "multiple_tier" => Ok(Self::MultipleTier),
            _ => Err(format!("Unknown LoyaltyProgramType variant: {}", s)),
        }
    }
}

impl Default for LoyaltyProgramType {
    fn default() -> Self {
        Self::SingleTier
    }
}
