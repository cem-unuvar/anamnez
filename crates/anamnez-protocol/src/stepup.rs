//! `StepUpAction` — wire form. Tags match `auth::stepup::StepUpAction::as_str()`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StepUpAction {
    #[serde(rename = "user.create")]
    UserCreate,
    #[serde(rename = "user.modify")]
    UserModify,
    #[serde(rename = "patient_access.grant_to_new_user")]
    PatientAccessGrantToNewUser,
    #[serde(rename = "user.disable")]
    UserDisable,
    #[serde(rename = "workstation.revoke")]
    WorkstationRevoke,
    #[serde(rename = "patient.export")]
    PatientDossierExport,
    #[serde(rename = "query.large_download")]
    LargeQueryDownload,
    #[serde(rename = "retention.policy_change")]
    RetentionPolicyChange,
    #[serde(rename = "workstation.enroll")]
    WorkstationEnrollmentString,
}
