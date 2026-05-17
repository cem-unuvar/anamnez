//! Ownership transfer at user disable (README §Compliance → KVKK-derived features).

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{PatientId, UserId};
use crate::patient_access;
use rusqlite::params;
use serde_json::json;

pub fn sole_owned_patients(db: &Database, user_id: UserId) -> Result<Vec<PatientId>> {
    db.with_reader(|conn| {
        let mut stmt = conn.prepare(
            "SELECT patient_id FROM patient_access WHERE user_id = ?1 AND level = 'owner'",
        )?;
        let owned: Vec<String> = stmt
            .query_map(params![user_id.as_uuid().to_string()], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::new();
        for pid in owned {
            let uuid =
                uuid::Uuid::parse_str(&pid).map_err(|_| Error::Invariant("patient_id parse"))?;
            out.push(PatientId(uuid));
        }
        Ok(out)
    })
}

pub fn disable_user_with_successors(
    db: &Database,
    admin: UserId,
    target: UserId,
    successors: Vec<(PatientId, UserId)>,
) -> Result<()> {
    let sole = sole_owned_patients(db, target)?;
    let map: std::collections::HashMap<PatientId, UserId> = successors.into_iter().collect();
    for p in &sole {
        if !map.contains_key(p) {
            return Err(Error::SoleOwnerOfPatient {
                patient_id: p.as_uuid().to_string(),
            });
        }
    }
    for (patient_id, successor) in map.iter() {
        patient_access::transfer_ownership(db, admin, *patient_id, target, *successor)?;
    }
    let now = db.clock().now();
    db.with_writer(|conn| {
        conn.execute(
            "UPDATE user SET disabled_at = ?2 WHERE id = ?1",
            params![target.as_uuid().to_string(), now.to_string()],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::UserDisable,
                target_type: "user".into(),
                target_id: target.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"reassigned": sole.len()}),
            },
        )?;
        Ok(())
    })
}
