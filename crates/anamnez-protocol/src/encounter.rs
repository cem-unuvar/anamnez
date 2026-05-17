use crate::codesystem::CodeSystem;
use crate::ids::{EncounterId, PatientId, UserId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterKind {
    InPerson,
    Phone,
    Video,
    AsyncDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterStatus {
    InProgress,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encounter {
    pub id: EncounterId,
    pub patient_id: PatientId,
    pub provider_id: UserId,
    pub kind: EncounterKind,
    pub reason_text: String,
    pub reason_code: Option<String>,
    pub reason_code_system: Option<CodeSystem>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub status: EncounterStatus,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartEncounterRequest {
    pub patient_id: PatientId,
    pub kind: EncounterKind,
    pub reason_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishEncounterRequest {
    pub expected_version: i64,
    pub reason_code: String,
    pub reason_code_system: CodeSystem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelEncounterRequest {
    pub expected_version: i64,
}
