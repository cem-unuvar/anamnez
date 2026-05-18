//! `GET /v1/health` wire envelope. Distinct from `anamnez_core::health::HealthReport`,
//! which is a richer in-process probe used by CLI / launchd. The wire shape is
//! intentionally lean — the only fields a workstation cares about are status and the
//! environment flag (which drives the TEST shield even before login).

use serde::{Deserialize, Serialize};

use crate::environment::Environment;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEnvelope {
    pub status: String,
    pub version: String,
    pub environment: Environment,
}
