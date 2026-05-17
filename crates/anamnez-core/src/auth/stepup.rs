//! Step-up reauthentication — README §Wire protocol.

use crate::audit::{self, Action, AppendInput};
use crate::auth::password;
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::UserId;
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use secrecy::SecretString;
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepUpAction {
    UserCreate,
    UserModify,
    PatientAccessGrantToNewUser,
    UserDisable,
    WorkstationRevoke,
    PatientDossierExport,
    LargeQueryDownload,
    RetentionPolicyChange,
    WorkstationEnrollmentString,
}

impl StepUpAction {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserCreate => "user.create",
            Self::UserModify => "user.modify",
            Self::PatientAccessGrantToNewUser => "patient_access.grant_to_new_user",
            Self::UserDisable => "user.disable",
            Self::WorkstationRevoke => "workstation.revoke",
            Self::PatientDossierExport => "patient.export",
            Self::LargeQueryDownload => "query.large_download",
            Self::RetentionPolicyChange => "retention.policy_change",
            Self::WorkstationEnrollmentString => "workstation.enroll",
        }
    }
}

/// Verify the user's re-entered password and authorize a step-up action.
/// Audits both success and failure.
pub fn verify_for(
    db: &Database,
    user_id: UserId,
    action: StepUpAction,
    password: SecretString,
) -> Result<StepUpReceipt> {
    let now = db.clock().now();
    db.with_writer(|conn| {
        let hash: Option<String> = conn
            .query_row(
                "SELECT password_hash FROM user WHERE id = ?1 AND disabled_at IS NULL",
                params![user_id.as_uuid().to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let hash = hash.ok_or(Error::BadCredentials)?;
        let ok = password::verify(password, &hash)?;
        let outcome = if ok { "success" } else { "failure" };
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(user_id),
                auth_session_id: None,
                action: Action::UserLogin, // No dedicated `stepup.verify` action in spec; reuse UserLogin with metadata.
                target_type: "stepup".into(),
                target_id: action.as_str().to_owned(),
                patient_id: None,
                metadata: json!({"action": action.as_str(), "outcome": outcome}),
            },
        )?;
        if !ok {
            return Err(Error::StepUpRequired {
                action: action.as_str(),
            });
        }
        Ok(StepUpReceipt {
            user_id,
            action,
            issued_at: now,
        })
    })
}

/// Returned on a successful step-up. Single-use; consumed when the protected action runs.
#[derive(Debug, Clone)]
pub struct StepUpReceipt {
    pub user_id: UserId,
    pub action: StepUpAction,
    pub issued_at: Timestamp,
}
