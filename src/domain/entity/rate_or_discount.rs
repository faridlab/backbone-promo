use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "rate_or_discount", rename_all = "snake_case")]
pub enum RateOrDiscount {
    Rate,
    DiscountPercentage,
    DiscountAmount,
}

impl std::fmt::Display for RateOrDiscount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rate => write!(f, "rate"),
            Self::DiscountPercentage => write!(f, "discount_percentage"),
            Self::DiscountAmount => write!(f, "discount_amount"),
        }
    }
}

impl FromStr for RateOrDiscount {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rate" => Ok(Self::Rate),
            "discount_percentage" => Ok(Self::DiscountPercentage),
            "discount_amount" => Ok(Self::DiscountAmount),
            _ => Err(format!("Unknown RateOrDiscount variant: {}", s)),
        }
    }
}

impl Default for RateOrDiscount {
    fn default() -> Self {
        Self::DiscountPercentage
    }
}
