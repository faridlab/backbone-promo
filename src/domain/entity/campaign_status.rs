use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "campaign_status", rename_all = "snake_case")]
pub enum CampaignStatus {
    Draft,
    Scheduled,
    Active,
    Paused,
    Ended,
    Cancelled,
}

impl std::fmt::Display for CampaignStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Scheduled => write!(f, "scheduled"),
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Ended => write!(f, "ended"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for CampaignStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "scheduled" => Ok(Self::Scheduled),
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "ended" => Ok(Self::Ended),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown CampaignStatus variant: {}", s)),
        }
    }
}

impl Default for CampaignStatus {
    fn default() -> Self {
        Self::Draft
    }
}
