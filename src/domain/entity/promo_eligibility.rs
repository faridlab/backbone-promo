use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "promo_eligibility", rename_all = "snake_case")]
pub enum PromoEligibility {
    All,
    NewCustomer,
    ReturningCustomer,
    LoyaltyMember,
    SpecificTiers,
}

impl std::fmt::Display for PromoEligibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::NewCustomer => write!(f, "new_customer"),
            Self::ReturningCustomer => write!(f, "returning_customer"),
            Self::LoyaltyMember => write!(f, "loyalty_member"),
            Self::SpecificTiers => write!(f, "specific_tiers"),
        }
    }
}

impl FromStr for PromoEligibility {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(Self::All),
            "new_customer" => Ok(Self::NewCustomer),
            "returning_customer" => Ok(Self::ReturningCustomer),
            "loyalty_member" => Ok(Self::LoyaltyMember),
            "specific_tiers" => Ok(Self::SpecificTiers),
            _ => Err(format!("Unknown PromoEligibility variant: {}", s)),
        }
    }
}

impl Default for PromoEligibility {
    fn default() -> Self {
        Self::All
    }
}
