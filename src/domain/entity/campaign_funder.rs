use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "campaign_funder", rename_all = "snake_case")]
pub enum CampaignFunder {
    Platform,
    Provider,
    Shared,
}

impl std::fmt::Display for CampaignFunder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Platform => write!(f, "platform"),
            Self::Provider => write!(f, "provider"),
            Self::Shared => write!(f, "shared"),
        }
    }
}

impl FromStr for CampaignFunder {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "platform" => Ok(Self::Platform),
            "provider" => Ok(Self::Provider),
            "shared" => Ok(Self::Shared),
            _ => Err(format!("Unknown CampaignFunder variant: {}", s)),
        }
    }
}

impl Default for CampaignFunder {
    fn default() -> Self {
        Self::Platform
    }
}
