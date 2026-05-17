use crate::codesystem::CodeSystem;
use crate::ids::{AllergyId, EncounterId, PatientId, SourceDocumentId, UserId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllergySeverity {
    Mild,
    Moderate,
    Severe,
    LifeThreatening,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllergyStatus {
    Active,
    Inactive,
    EnteredInError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allergy {
    pub id: AllergyId,
    pub patient_id: PatientId,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub severity: AllergySeverity,
    pub reaction_text: Option<String>,
    pub status: AllergyStatus,
    pub onset_date: Option<jiff::civil::Date>,
    pub recorded_at: Timestamp,
    pub recorded_by: UserId,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAllergy {
    pub patient_id: PatientId,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub severity: AllergySeverity,
    pub reaction_text: Option<String>,
    pub status: AllergyStatus,
    pub onset_date: Option<jiff::civil::Date>,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AllergyPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<AllergySeverity>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub reaction_text: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<AllergyStatus>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub onset_date: Option<Option<jiff::civil::Date>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmendAllergyRequest {
    pub expected_version: i64,
    pub patch: AllergyPatch,
}
