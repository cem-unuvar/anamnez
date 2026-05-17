use crate::ids::{PatientId, UserId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SexAssignedAtBirth {
    Female,
    Male,
    Intersex,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    pub id: PatientId,
    pub mrn: Option<String>,
    pub given_names: String,
    pub family_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: jiff::civil::Date,
    pub sex_assigned_at_birth: SexAssignedAtBirth,
    pub gender_identity: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub created_by: UserId,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deceased_at: Option<Timestamp>,
    pub archived_at: Option<Timestamp>,
    pub suppressed_at: Option<Timestamp>,
    pub suppression_reason: Option<String>,
    pub notice_acknowledged_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPatient {
    pub mrn: Option<String>,
    pub given_names: String,
    pub family_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: jiff::civil::Date,
    pub sex_assigned_at_birth: SexAssignedAtBirth,
    pub gender_identity: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relationship: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatientPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub mrn: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub preferred_name: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub email: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub phone: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub address: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub deceased_at: Option<Option<Timestamp>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub archived_at: Option<Option<Timestamp>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub notice_acknowledged_at: Option<Option<Timestamp>>,
}

/// Wire envelope for PATCH /v1/patients/:id and similar mutators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePatientRequest {
    pub expected_version: i64,
    pub patch: PatientPatch,
}
