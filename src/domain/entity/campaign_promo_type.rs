use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "campaign_promo_type", rename_all = "snake_case")]
pub enum CampaignPromoType {
    Percentage,
    FixedAmount,
    FreeDelivery,
    FreeItem,
    Cashback,
    Bundle,
}

impl std::fmt::Display for CampaignPromoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Percentage => write!(f, "percentage"),
            Self::FixedAmount => write!(f, "fixed_amount"),
            Self::FreeDelivery => write!(f, "free_delivery"),
            Self::FreeItem => write!(f, "free_item"),
            Self::Cashback => write!(f, "cashback"),
            Self::Bundle => write!(f, "bundle"),
        }
    }
}

impl FromStr for CampaignPromoType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "percentage" => Ok(Self::Percentage),
            "fixed_amount" => Ok(Self::FixedAmount),
            "free_delivery" => Ok(Self::FreeDelivery),
            "free_item" => Ok(Self::FreeItem),
            "cashback" => Ok(Self::Cashback),
            "bundle" => Ok(Self::Bundle),
            _ => Err(format!("Unknown CampaignPromoType variant: {}", s)),
        }
    }
}

impl Default for CampaignPromoType {
    fn default() -> Self {
        Self::Percentage
    }
}
