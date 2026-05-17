//! Patient dossier export (KVKK m. 11/b + Hasta Hakları Yön. m. 42).

use crate::analysis::json_shape;
use crate::audit::{self, Action, AppendInput};
use crate::auth::stepup::{StepUpAction, StepUpReceipt};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::PatientId;
use serde_json::{json, Value};

/// Build the dossier payload — demographics, problem list, allergies, medications,
/// encounters timeline, observations grouped by encounter, source-document attachment refs.
/// Requires a valid `StepUpReceipt` for `PatientDossierExport`; emits `patient.export` audit.
pub fn export(db: &Database, patient_id: PatientId, receipt: StepUpReceipt) -> Result<Value> {
    if !matches!(receipt.action, StepUpAction::PatientDossierExport) {
        return Err(Error::StepUpRequired {
            action: "patient.export",
        });
    }
    let payload = json_shape::build(db, patient_id)?;
    audit::append(
        db,
        AppendInput {
            actor_user_id: Some(receipt.user_id),
            auth_session_id: None,
            action: Action::PatientExport,
            target_type: "patient".into(),
            target_id: patient_id.as_uuid().to_string(),
            patient_id: Some(patient_id),
            metadata: json!({"step_up_issued_at": receipt.issued_at.to_string()}),
        },
    )?;
    Ok(payload)
}
