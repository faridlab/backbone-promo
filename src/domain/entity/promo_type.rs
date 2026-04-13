use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "promo_type", rename_all = "snake_case")]
pub enum PromoType {
    Percentage,
    FixedAmount,
    FreeDelivery,
    FreePickup,
    BuyXGetY,
    Cashback,
}

impl std::fmt::Display for PromoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Percentage => write!(f, "percentage"),
            Self::FixedAmount => write!(f, "fixed_amount"),
            Self::FreeDelivery => write!(f, "free_delivery"),
            Self::FreePickup => write!(f, "free_pickup"),
            Self::BuyXGetY => write!(f, "buy_x_get_y"),
            Self::Cashback => write!(f, "cashback"),
        }
    }
}

impl FromStr for PromoType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "percentage" => Ok(Self::Percentage),
            "fixed_amount" => Ok(Self::FixedAmount),
            "free_delivery" => Ok(Self::FreeDelivery),
            "free_pickup" => Ok(Self::FreePickup),
            "buy_x_get_y" => Ok(Self::BuyXGetY),
            "cashback" => Ok(Self::Cashback),
            _ => Err(format!("Unknown PromoType variant: {}", s)),
        }
    }
}

impl Default for PromoType {
    fn default() -> Self {
        Self::Percentage
    }
}
