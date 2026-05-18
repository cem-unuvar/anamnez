//! Wire mirror of `anamnez_core::env::Environment`. Lives in protocol so the workstation
//! (wasm32) can see it without pulling in `anamnez-core`.

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
    pub fn is_test(self) -> bool {
        matches!(self, Self::Test)
    }
}
