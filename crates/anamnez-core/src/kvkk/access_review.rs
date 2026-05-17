//! Periodic access review (README §Compliance → KVKK-derived features).
//!
//! "Lists `patient_access` rows whose user has not touched the patient (via `audit_log`)
//! in ≥6 months."

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::Result;
use crate::ids::{PatientId, UserId};
use crate::patient_access::AccessLevel;
use jiff::Timestamp;
use rusqlite::params;
use serde_json::json;

const SIX_MONTHS_SECONDS: u64 = 60 * 60 * 24 * 30 * 6;

#[derive(Debug, Clone)]
pub struct SilentAccessRow {
    pub patient_id: PatientId,
    pub user_id: UserId,
    pub level: AccessLevel,
    pub last_touched_at: Option<Timestamp>,
}

pub fn silent_grants(db: &Database) -> Result<Vec<SilentAccessRow>> {
    let now = db.clock().now();
    let cutoff = now
        .checked_sub(std::time::Duration::from_secs(SIX_MONTHS_SECONDS))
        .map_err(|_| crate::error::Error::Invariant("cutoff overflow"))?;
    db.with_reader(|conn| {
        let mut stmt = conn.prepare(
            "SELECT pa.patient_id, pa.user_id, pa.level, \
                    (SELECT MAX(al.occurred_at) FROM audit_log al \
                       WHERE al.patient_id = pa.patient_id AND al.actor_user_id = pa.user_id) AS last_touched \
             FROM patient_access pa",
        )?;
        let rows = stmt
            .query_map(params![], |r| {
                let pid: String = r.get(0)?;
                let uid: String = r.get(1)?;
                let lvl: String = r.get(2)?;
                let last_touched: Option<String> = r.get(3)?;
                Ok((pid, uid, lvl, last_touched))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut out = Vec::new();
        for (pid, uid, lvl, last_touched_str) in rows {
            let last_touched = match last_touched_str.as_deref() {
                None => None,
                Some(s) => Some(
                    s.parse::<Timestamp>()
                        .map_err(|_| crate::error::Error::Invariant("audit occurred_at parse"))?,
                ),
            };
            let is_silent = match last_touched {
                None => true,
                Some(t) => t < cutoff,
            };
            if is_silent {
                let p_uuid = uuid::Uuid::parse_str(&pid)
                    .map_err(|_| crate::error::Error::Invariant("patient_id parse"))?;
                let u_uuid = uuid::Uuid::parse_str(&uid)
                    .map_err(|_| crate::error::Error::Invariant("user_id parse"))?;
                out.push(SilentAccessRow {
                    patient_id: PatientId(p_uuid),
                    user_id: UserId(u_uuid),
                    level: AccessLevel::parse(&lvl)?,
                    last_touched_at: last_touched,
                });
            }
        }
        Ok(out)
    })
}

pub fn mark_completed(db: &Database, admin: UserId) -> Result<()> {
    let now = db.clock().now();
    audit::append(
        db,
        AppendInput {
            actor_user_id: Some(admin),
            auth_session_id: None,
            action: Action::AccessReviewCompleted,
            target_type: "access_review".into(),
            target_id: now.to_string(),
            patient_id: None,
            metadata: json!({}),
        },
    )?;
    Ok(())
}
