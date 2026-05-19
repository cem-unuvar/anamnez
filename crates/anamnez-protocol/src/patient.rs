use crate::access::AccessLevel;
use crate::allergy::Allergy;
use crate::encounter::Encounter;
use crate::ids::{PatientId, UserId};
use crate::medication::Medication;
use crate::observation::Observation;
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

/// One row in the patient list. Lighter than `Patient` — only what the list view renders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientListItem {
    pub id: PatientId,
    pub mrn: Option<String>,
    pub given_names: String,
    pub family_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: jiff::civil::Date,
    pub sex_assigned_at_birth: SexAssignedAtBirth,
    /// The caller's access level on this patient. Always populated (a row is only listed
    /// when the caller has a `patient_access` entry).
    pub access_level: AccessLevel,
    pub updated_at: Timestamp,
    pub deceased_at: Option<Timestamp>,
    pub archived_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatientListQuery {
    /// Free-text filter applied case-insensitively against given_names/family_name/mrn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// If true, include `archived_at IS NOT NULL` rows. Default false. Suppressed rows
    /// are never returned regardless of this flag.
    #[serde(default)]
    pub include_archived: bool,
    /// Pagination cursor — the `updated_at` of the last item from the previous page.
    /// First page omits this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Timestamp>,
    /// Max rows to return. The server caps this at its own ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientListResponse {
    pub items: Vec<PatientListItem>,
    /// Cursor to pass as `before` for the next page. `None` if this is the last page.
    pub next_before: Option<Timestamp>,
}

/// Bundled read-only patient view: demographics + active problems + allergies +
/// medications + encounters + (if a visit is in progress) the observations
/// recorded during it. One round-trip, one consistent snapshot. Encounters
/// and active-visit observations carry their `Versioned` wrapper so the UI
/// can amend, finish, or mark-entered-in-error without a second round-trip
/// to fetch versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientDetail {
    pub patient: Patient,
    pub access_level: AccessLevel,
    pub problem_list: Vec<Observation>,
    pub allergies: Vec<Allergy>,
    pub medications: Vec<Medication>,
    pub encounters: Vec<crate::versioned::Versioned<Encounter>>,
    /// Observations recorded against the currently in-progress encounter, newest
    /// first. Empty when no visit is open. `entered_in_error` rows are excluded.
    #[serde(default)]
    pub active_encounter_observations: Vec<crate::versioned::Versioned<Observation>>,
}
