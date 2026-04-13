use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "ad_type", rename_all = "snake_case")]
pub enum AdType {
    Banner,
    Featured,
    Promoted,
    Popup,
    Push,
    Sms,
    Email,
}

impl std::fmt::Display for AdType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Banner => write!(f, "banner"),
            Self::Featured => write!(f, "featured"),
            Self::Promoted => write!(f, "promoted"),
            Self::Popup => write!(f, "popup"),
            Self::Push => write!(f, "push"),
            Self::Sms => write!(f, "sms"),
            Self::Email => write!(f, "email"),
        }
    }
}

impl FromStr for AdType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "banner" => Ok(Self::Banner),
            "featured" => Ok(Self::Featured),
            "promoted" => Ok(Self::Promoted),
            "popup" => Ok(Self::Popup),
            "push" => Ok(Self::Push),
            "sms" => Ok(Self::Sms),
            "email" => Ok(Self::Email),
            _ => Err(format!("Unknown AdType variant: {}", s)),
        }
    }
}

impl Default for AdType {
    fn default() -> Self {
        Self::Banner
    }
}
