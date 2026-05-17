use crate::ids::{PatientConsentId, PatientId, SourceDocumentId, UserId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentPurpose {
    LawyerTransfer,
    ResearchNonAnonymized,
    OtherClinicReferral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientConsent {
    pub id: PatientConsentId,
    pub patient_id: PatientId,
    pub purpose: ConsentPurpose,
    pub granted_at: Timestamp,
    pub granted_by: UserId,
    pub evidence_source_id: Option<SourceDocumentId>,
    pub revoked_at: Option<Timestamp>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordConsentRequest {
    pub patient_id: PatientId,
    pub purpose: ConsentPurpose,
    pub evidence_source_id: Option<SourceDocumentId>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeConsentRequest {
    pub expected_version: i64,
}
