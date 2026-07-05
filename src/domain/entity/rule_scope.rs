use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "rule_scope", rename_all = "snake_case")]
pub enum RuleScope {
    Line,
    Order,
}

impl std::fmt::Display for RuleScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Line => write!(f, "line"),
            Self::Order => write!(f, "order"),
        }
    }
}

impl FromStr for RuleScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "line" => Ok(Self::Line),
            "order" => Ok(Self::Order),
            _ => Err(format!("Unknown RuleScope variant: {}", s)),
        }
    }
}

impl Default for RuleScope {
    fn default() -> Self {
        Self::Line
    }
}
