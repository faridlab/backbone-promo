use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "bundle_match", rename_all = "snake_case")]
pub enum BundleMatch {
    AllOf,
    AnyN,
}

impl std::fmt::Display for BundleMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllOf => write!(f, "all_of"),
            Self::AnyN => write!(f, "any_n"),
        }
    }
}

impl FromStr for BundleMatch {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all_of" => Ok(Self::AllOf),
            "any_n" => Ok(Self::AnyN),
            _ => Err(format!("Unknown BundleMatch variant: {}", s)),
        }
    }
}

impl Default for BundleMatch {
    fn default() -> Self {
        Self::AllOf
    }
}
