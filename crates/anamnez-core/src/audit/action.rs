//! `Action` enum — closed (no `serde(other)`). Adding an action requires a SPEC.md PR.

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

impl Action {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PatientView => "patient.view",
            Self::PatientUpdate => "patient.update",
            Self::PatientExport => "patient.export",
            Self::PatientOwnershipTransfer => "patient.ownership_transfer",
            Self::ObservationCreate => "observation.create",
            Self::ObservationAmend => "observation.amend",
            Self::AllergyCreate => "allergy.create",
            Self::AllergyAmend => "allergy.amend",
            Self::MedicationCreate => "medication.create",
            Self::MedicationAmend => "medication.amend",
            Self::SourceDocumentCreate => "source_document.create",
            Self::ConsentRecord => "consent.record",
            Self::ConsentRevoke => "consent.revoke",
            Self::EncounterStart => "encounter.start",
            Self::EncounterFinish => "encounter.finish",
            Self::EncounterCancel => "encounter.cancel",
            Self::UserLogin => "user.login",
            Self::UserCreate => "user.create",
            Self::UserModify => "user.modify",
            Self::UserDisable => "user.disable",
            Self::WorkstationEnroll => "workstation.enroll",
            Self::WorkstationRevoke => "workstation.revoke",
            Self::PatientAccessGrant => "patient_access.grant",
            Self::PatientAccessRevoke => "patient_access.revoke",
            Self::AnalysisGenerate => "analysis.generate",
            Self::CodesystemsUpdate => "codesystems.update",
            Self::AccessReviewCompleted => "access_review.completed",
            Self::RetentionSweep => "retention_sweep",
        }
    }
}
