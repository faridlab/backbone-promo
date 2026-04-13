use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "campaign_target", rename_all = "snake_case")]
pub enum CampaignTarget {
    All,
    NewUsers,
    Returning,
    Inactive,
    Specific,
    Loyal,
}

impl std::fmt::Display for CampaignTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::NewUsers => write!(f, "new_users"),
            Self::Returning => write!(f, "returning"),
            Self::Inactive => write!(f, "inactive"),
            Self::Specific => write!(f, "specific"),
            Self::Loyal => write!(f, "loyal"),
        }
    }
}

impl FromStr for CampaignTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(Self::All),
            "new_users" => Ok(Self::NewUsers),
            "returning" => Ok(Self::Returning),
            "inactive" => Ok(Self::Inactive),
            "specific" => Ok(Self::Specific),
            "loyal" => Ok(Self::Loyal),
            _ => Err(format!("Unknown CampaignTarget variant: {}", s)),
        }
    }
}

impl Default for CampaignTarget {
    fn default() -> Self {
        Self::All
    }
}
