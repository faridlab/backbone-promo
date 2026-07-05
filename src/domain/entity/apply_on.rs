use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "apply_on", rename_all = "snake_case")]
pub enum ApplyOn {
    Item,
    ItemGroup,
    Brand,
    All,
}

impl std::fmt::Display for ApplyOn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Item => write!(f, "item"),
            Self::ItemGroup => write!(f, "item_group"),
            Self::Brand => write!(f, "brand"),
            Self::All => write!(f, "all"),
        }
    }
}

impl FromStr for ApplyOn {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "item" => Ok(Self::Item),
            "item_group" => Ok(Self::ItemGroup),
            "brand" => Ok(Self::Brand),
            "all" => Ok(Self::All),
            _ => Err(format!("Unknown ApplyOn variant: {}", s)),
        }
    }
}

impl Default for ApplyOn {
    fn default() -> Self {
        Self::Item
    }
}
