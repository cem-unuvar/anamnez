use crate::codesystem::CodeSystem;
use crate::ids::{EncounterId, MedicationId, PatientId, SourceDocumentId, UserId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicationRoute {
    Oral,
    Iv,
    Im,
    Topical,
    Inhaled,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicationStatus {
    Active,
    Completed,
    Stopped,
    EnteredInError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medication {
    pub id: MedicationId,
    pub patient_id: PatientId,
    pub code: String,
    pub code_system: CodeSystem,
    pub display_text: String,
    pub dose_quantity: Option<f64>,
    pub dose_unit: Option<String>,
    pub frequency_text: Option<String>,
    pub route: MedicationRoute,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub reason_text: Option<String>,
    pub status: MedicationStatus,
    pub prescriber_id: Option<UserId>,
    pub recorded_at: Timestamp,
    pub recorded_by: UserId,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMedication {
    pub patient_id: PatientId,
    pub code: String,
    pub code_system: CodeSystem,
    pub display_text: String,
    pub dose_quantity: Option<f64>,
    pub dose_unit: Option<String>,
    pub frequency_text: Option<String>,
    pub route: MedicationRoute,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub reason_text: Option<String>,
    pub status: MedicationStatus,
    pub prescriber_id: Option<UserId>,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MedicationPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub dose_quantity: Option<Option<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub dose_unit: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub frequency_text: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub ended_at: Option<Option<Timestamp>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<MedicationStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmendMedicationRequest {
    pub expected_version: i64,
    pub patch: MedicationPatch,
}
