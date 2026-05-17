//! Erasure-via-suppression workflow (KVKK m. 11/e).

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{PatientId, UserId};
use rusqlite::params;
use serde_json::json;

/// Mark a patient as suppressed with a justification.
/// Suppressed rows are invisible everywhere except `audit_log` and the retention sweep.
pub fn suppress(db: &Database, actor: UserId, patient_id: PatientId, reason: String) -> Result<()> {
    let now = db.clock().now();
    db.with_writer(|conn| {
        let affected = conn.execute(
            "UPDATE patient SET suppressed_at = ?2, suppression_reason = ?3 \
             WHERE id = ?1 AND suppressed_at IS NULL",
            params![patient_id.as_uuid().to_string(), now.to_string(), reason],
        )?;
        if affected == 0 {
            return Err(Error::NotFound);
        }
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::PatientView,
                target_type: "patient".into(),
                target_id: patient_id.as_uuid().to_string(),
                patient_id: Some(patient_id),
                metadata: json!({"op": "suppress", "reason": reason}),
            },
        )?;
        Ok(())
    })
}

pub fn is_suppressed(db: &Database, patient_id: PatientId) -> Result<bool> {
    db.with_reader(|conn| {
        let s: Option<String> = conn.query_row(
            "SELECT suppressed_at FROM patient WHERE id = ?1",
            params![patient_id.as_uuid().to_string()],
            |r| r.get(0),
        )?;
        Ok(s.is_some())
    })
}
