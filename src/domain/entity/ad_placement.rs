use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "ad_placement", rename_all = "snake_case")]
pub enum AdPlacement {
    HomeTop,
    HomeMiddle,
    SearchResults,
    ServiceDetail,
    Checkout,
    OrderComplete,
}

impl std::fmt::Display for AdPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HomeTop => write!(f, "home_top"),
            Self::HomeMiddle => write!(f, "home_middle"),
            Self::SearchResults => write!(f, "search_results"),
            Self::ServiceDetail => write!(f, "service_detail"),
            Self::Checkout => write!(f, "checkout"),
            Self::OrderComplete => write!(f, "order_complete"),
        }
    }
}

impl FromStr for AdPlacement {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "home_top" => Ok(Self::HomeTop),
            "home_middle" => Ok(Self::HomeMiddle),
            "search_results" => Ok(Self::SearchResults),
            "service_detail" => Ok(Self::ServiceDetail),
            "checkout" => Ok(Self::Checkout),
            "order_complete" => Ok(Self::OrderComplete),
            _ => Err(format!("Unknown AdPlacement variant: {}", s)),
        }
    }
}

impl Default for AdPlacement {
    fn default() -> Self {
        Self::HomeTop
    }
}
