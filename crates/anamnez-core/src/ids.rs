//! Strongly-typed newtypes for the primary identifiers in the data model.
//!
//! README data-model rows carry `id` columns; we use UUID v4 as the canonical
//! representation across the system to avoid sequence-leak side channels.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident, $tag:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $tag, self.0)
            }
        }
    };
}

id_type!(UserId, "user");
id_type!(PatientId, "patient");
id_type!(EncounterId, "encounter");
id_type!(ObservationId, "obs");
id_type!(AllergyId, "allergy");
id_type!(MedicationId, "med");
id_type!(SourceDocumentId, "src");
id_type!(ExtractionId, "ext");
id_type!(PatientAnalysisId, "analysis");
id_type!(PatientConsentId, "consent");
id_type!(WorkstationId, "ws");
id_type!(AuthSessionId, "session");

/// `audit_log.id` is an `i64` rather than a UUID — the audit chain hashes `id` into
/// every row, and a monotonic sequence makes verification ordering straightforward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuditLogId(pub i64);

impl AuditLogId {
    #[must_use]
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

impl fmt::Display for AuditLogId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit:{}", self.0)
    }
}
