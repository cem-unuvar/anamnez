use crate::codesystem::CodeSystem;
use crate::ids::{EncounterId, ObservationId, PatientId, SourceDocumentId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    Preliminary,
    Final,
    Amended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractedBy {
    Manual,
    Llm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueQuantity {
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObservationValue {
    Quantity(ValueQuantity),
    String(String),
    Codeable {
        code_system: CodeSystem,
        code: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub patient_id: PatientId,
    pub recorded_at: Timestamp,
    pub effective_period_start: Timestamp,
    pub effective_period_end: Option<Timestamp>,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub value: Option<ObservationValue>,
    pub status: ObservationStatus,
    pub is_problem_list_item: bool,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
    pub extracted_by: ExtractedBy,
    pub model_version: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewObservation {
    pub patient_id: PatientId,
    pub effective_period_start: Timestamp,
    pub effective_period_end: Option<Timestamp>,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub value: Option<ObservationValue>,
    pub status: ObservationStatus,
    pub is_problem_list_item: bool,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
    pub extracted_by: ExtractedBy,
    pub model_version: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservationPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub effective_period_end: Option<Option<Timestamp>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub code: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub code_system: Option<Option<CodeSystem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_text: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub value: Option<Option<ObservationValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ObservationStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_problem_list_item: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmendObservationRequest {
    pub expected_version: i64,
    pub patch: ObservationPatch,
}
