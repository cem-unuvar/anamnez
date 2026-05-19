//! `Action` and `UserRole` — closed enums, identical wire tags to core.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "patient.view")]
    PatientView,
    #[serde(rename = "patient.update")]
    PatientUpdate,
    #[serde(rename = "patient.export")]
    PatientExport,
    #[serde(rename = "patient.ownership_transfer")]
    PatientOwnershipTransfer,
    #[serde(rename = "observation.create")]
    ObservationCreate,
    #[serde(rename = "observation.amend")]
    ObservationAmend,
    #[serde(rename = "observation.entered_in_error")]
    ObservationEnteredInError,
    #[serde(rename = "allergy.create")]
    AllergyCreate,
    #[serde(rename = "allergy.amend")]
    AllergyAmend,
    #[serde(rename = "medication.create")]
    MedicationCreate,
    #[serde(rename = "medication.amend")]
    MedicationAmend,
    #[serde(rename = "source_document.create")]
    SourceDocumentCreate,
    #[serde(rename = "consent.record")]
    ConsentRecord,
    #[serde(rename = "consent.revoke")]
    ConsentRevoke,
    #[serde(rename = "encounter.start")]
    EncounterStart,
    #[serde(rename = "encounter.finish")]
    EncounterFinish,
    #[serde(rename = "encounter.cancel")]
    EncounterCancel,
    #[serde(rename = "user.login")]
    UserLogin,
    #[serde(rename = "user.create")]
    UserCreate,
    #[serde(rename = "user.modify")]
    UserModify,
    #[serde(rename = "user.disable")]
    UserDisable,
    #[serde(rename = "workstation.enroll")]
    WorkstationEnroll,
    #[serde(rename = "workstation.revoke")]
    WorkstationRevoke,
    #[serde(rename = "patient_access.grant")]
    PatientAccessGrant,
    #[serde(rename = "patient_access.revoke")]
    PatientAccessRevoke,
    #[serde(rename = "analysis.generate")]
    AnalysisGenerate,
    #[serde(rename = "codesystems.update")]
    CodesystemsUpdate,
    #[serde(rename = "access_review.completed")]
    AccessReviewCompleted,
    #[serde(rename = "retention_sweep")]
    RetentionSweep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    Provider,
}
