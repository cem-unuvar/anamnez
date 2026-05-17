//! SSE event union pushed on `GET /v1/events`. JSON tag: `kind` (snake_case).

use crate::access::AccessLevel;
use crate::ids::{ObservationId, PatientId, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEvent {
    pub id: u64,
    pub payload: ServerEventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerEventPayload {
    ObservationAmendedElsewhere {
        patient_id: PatientId,
        observation_id: ObservationId,
        by_user_id: UserId,
    },
    PatientAccessChanged {
        patient_id: PatientId,
        user_id: UserId,
        /// `None` if access was revoked entirely.
        level: Option<AccessLevel>,
    },
    ForcedLogout {
        reason: String,
    },
}
