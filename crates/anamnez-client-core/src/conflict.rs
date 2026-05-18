//! Conflict resolution scaffolding. The editor UIs that consume this ship in a later
//! slice (clinical CRUD); the seam is wired now so it doesn't disturb the typed-IPC
//! pass-through later.

use anamnez_protocol::error::ErrorEnvelope;
use serde::{Deserialize, Serialize};

use crate::error::ClientError;

/// Surface form: "record changed, here is the new state, reapply your change."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictNotice {
    pub current_version: i64,
    pub new_state_json: String,
}

impl ConflictNotice {
    /// Pull a conflict envelope out of `ClientError` if present; `None` otherwise.
    #[must_use]
    pub fn from_error(e: &ClientError) -> Option<Self> {
        match e {
            ClientError::Server(ErrorEnvelope::Conflict {
                current_version,
                new_state_json,
            }) => Some(Self {
                current_version: *current_version,
                new_state_json: new_state_json.clone(),
            }),
            _ => None,
        }
    }
}
