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
    EnteredInError,
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

/// New observation wire shape. `code` and `code_system` are **required** —
/// the SPEC's "preliminary with `code/code_system = null`" escape hatch has
/// been removed by tightening the UI and protocol. If autocomplete returns
/// no match, the clinician falls back to `ANAMNEZ-SYM` (catch-all symptoms).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewObservation {
    pub patient_id: PatientId,
    pub effective_period_start: Timestamp,
    pub effective_period_end: Option<Timestamp>,
    pub code: String,
    pub code_system: CodeSystem,
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

/// Slim shape for the workstation UI's manual-entry path. The WASM bundle does
/// not depend on jiff, so timestamps are filled in by the Tauri shell at the
/// `ui_create_observation` boundary. Code, status, and value semantics carry
/// straight through to [`NewObservation`].
///
/// `code` and `code_system` are **required** — every observation must carry
/// a code (`ANAMNEZ-SYM` is the catch-all when nothing else fits).
///
/// `value_quantity`/`value_unit` (lab results, vitals) and `value_text`
/// (qualitative findings like "negatif", "normal") are mutually exclusive and
/// both optional. The Tauri shell normalises them into the
/// [`ObservationValue`] enum before forwarding to the server — keeping the
/// wire shape flat means the WASM form doesn't have to construct enum
/// variants by hand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualObservationDraft {
    pub patient_id: PatientId,
    pub code: String,
    pub code_system: CodeSystem,
    pub display_text: String,
    pub status: ObservationStatus,
    pub is_problem_list_item: bool,
    pub encounter_id: Option<EncounterId>,
    #[serde(default)]
    pub value_quantity: Option<f64>,
    #[serde(default)]
    pub value_unit: Option<String>,
    #[serde(default)]
    pub value_text: Option<String>,
}

/// PATCH /v1/observations/:id/entered-in-error envelope. The endpoint expects
/// optimistic locking via `expected_version`. The server forces
/// `status = entered_in_error`, bumps the version, and broadcasts the
/// matching SSE event so other workstations drop the row from their views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkEnteredInErrorRequest {
    pub expected_version: i64,
}
