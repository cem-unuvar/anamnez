//! Crate-wide error type. Every public fallible API in `anamnez-core` returns `Result<T>`.

use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("conflict: record version mismatch (current version is {current_version})")]
    Conflict {
        current_version: i64,
        new_state_json: String,
    },

    #[error("not found")]
    NotFound,

    #[error("forbidden")]
    Forbidden,

    #[error("audit chain tampered at row {row_id}")]
    AuditTamper { row_id: i64 },

    #[error("invalid code: ({code_system}, {code}) not found in lookup tables")]
    CodeSystemMismatch { code_system: String, code: String },

    #[error("code system `{code_system}` is not valid for {context}")]
    CodeSystemNotAllowed {
        code_system: String,
        context: &'static str,
    },

    #[error("step-up reauthentication required for `{action}`")]
    StepUpRequired { action: &'static str },

    #[error("session revoked")]
    Revoked,

    #[error("client version `{got}` below minimum `{min}`")]
    OutdatedClient { min: String, got: String },

    #[error("environment marker mismatch: DB is `{db}`, daemon is `{daemon}`")]
    EnvironmentMarkerMismatch { db: String, daemon: String },

    #[error("schema version mismatch: DB has `{db}`, binary expects `{binary}`")]
    SchemaVersionMismatch { db: String, binary: String },

    #[error("invalid bundle signature")]
    InvalidBundleSignature,

    #[error("retired code `{code}` cannot be referenced by a new write")]
    RetiredCode { code: String },

    #[error("invalid state transition: {from} → {to}")]
    InvalidStateTransition {
        from: &'static str,
        to: &'static str,
    },

    #[error("test environment requires `[TEST]` prefix on patient names")]
    TestPrefixRequired,

    #[error("password verification failed")]
    BadCredentials,

    #[error("session expired (absolute_expires_at horizon reached)")]
    SessionExpired,

    #[error("user is sole owner of patient {patient_id} — designate successor first")]
    SoleOwnerOfPatient { patient_id: String },

    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("csv: {0}")]
    Csv(#[from] csv::Error),

    #[error("internal invariant: {0}")]
    Invariant(&'static str),
}
