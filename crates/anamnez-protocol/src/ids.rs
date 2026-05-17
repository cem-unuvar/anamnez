//! UUID-newtype wire IDs. `#[serde(transparent)]` — JSON is the bare UUID string.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            #[must_use]
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

id_type!(UserId);
id_type!(PatientId);
id_type!(EncounterId);
id_type!(ObservationId);
id_type!(AllergyId);
id_type!(MedicationId);
id_type!(SourceDocumentId);
id_type!(ExtractionId);
id_type!(PatientAnalysisId);
id_type!(PatientConsentId);
id_type!(WorkstationId);
id_type!(AuthSessionId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuditLogId(pub i64);
