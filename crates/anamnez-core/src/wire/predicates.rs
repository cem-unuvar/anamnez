//! Per-request session checks + min-client-version gate + step-up dispatcher.

use crate::auth::client_version::Version;
use crate::auth::stepup::StepUpAction;
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::AuthSessionId;
use rusqlite::{params, OptionalExtension};

/// Reject the request if the session has been revoked.
pub fn check_session_revoked(db: &Database, session_id: AuthSessionId) -> Result<()> {
    db.with_reader(|conn| {
        let revoked_at: Option<Option<String>> = conn
            .query_row(
                "SELECT revoked_at FROM auth_session WHERE id = ?1",
                params![session_id.as_uuid().to_string()],
                |r| r.get(0),
            )
            .optional()?;
        match revoked_at {
            None => Err(Error::NotFound),
            Some(Some(_)) => Err(Error::Revoked),
            Some(None) => Ok(()),
        }
    })
}

/// Return whether the given action requires step-up reauthentication.
#[must_use]
pub fn requires_stepup(action: &str) -> Option<StepUpAction> {
    match action {
        "user.create" => Some(StepUpAction::UserCreate),
        "user.modify" => Some(StepUpAction::UserModify),
        "patient_access.grant_to_new_user" => Some(StepUpAction::PatientAccessGrantToNewUser),
        "user.disable" => Some(StepUpAction::UserDisable),
        "workstation.revoke" => Some(StepUpAction::WorkstationRevoke),
        "patient.export" => Some(StepUpAction::PatientDossierExport),
        "query.large_download" => Some(StepUpAction::LargeQueryDownload),
        "retention.policy_change" => Some(StepUpAction::RetentionPolicyChange),
        "workstation.enroll" => Some(StepUpAction::WorkstationEnrollmentString),
        _ => None,
    }
}

/// Reject the request if the client's version is below the configured floor.
pub fn check_client_version(min: &Version, client_version_str: &str) -> Result<()> {
    let client = Version::parse(client_version_str)?;
    crate::auth::client_version::check(min, &client)
}
