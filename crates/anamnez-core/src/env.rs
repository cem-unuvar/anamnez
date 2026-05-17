//! README §Privacy — `Environment` enum gates OpenRouter access and test-mode safeguards.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    Production,
    Test,
}

impl Environment {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Test => "test",
        }
    }

    #[must_use]
    pub fn from_marker_str(s: &str) -> Option<Self> {
        match s {
            "production" => Some(Self::Production),
            "test" => Some(Self::Test),
            _ => None,
        }
    }
}
