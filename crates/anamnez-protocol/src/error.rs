//! Wire error envelope — discriminated union mirroring `anamnez_core::Error` minus
//! internal/boot-only variants. JSON tag: `kind` (snake_case).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorEnvelope {
    #[error("conflict: record version mismatch (current={current_version})")]
    Conflict {
        current_version: i64,
        new_state_json: String,
    },
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("bad credentials")]
    BadCredentials,
    #[error("session revoked")]
    Revoked,
    #[error("session expired")]
    SessionExpired,
    #[error("step-up required for `{action}`")]
    StepUpRequired { action: String },
    #[error("outdated client: got `{got}`, minimum `{min}`")]
    OutdatedClient { min: String, got: String },
    #[error("code system `{code_system}` not allowed for {context}")]
    CodeSystemNotAllowed {
        code_system: String,
        context: String,
    },
    #[error("code ({code_system}, {code}) not in lookup tables")]
    CodeSystemMismatch { code_system: String, code: String },
    #[error("retired code `{code}`")]
    RetiredCode { code: String },
    #[error("invalid state transition: {from} → {to}")]
    InvalidStateTransition { from: String, to: String },
    #[error("test environment requires `[TEST]` prefix on patient names")]
    TestPrefixRequired,
    #[error("user is sole owner of patient {patient_id}")]
    SoleOwnerOfPatient { patient_id: String },
    /// Collapses `Db`, `Io`, `Serde`, `Csv`, `Invariant`. Never carries sensitive payload.
    #[error("internal error")]
    Internal { detail: String },
}
