//! Retention sweep (README §Storage → Retention and destruction).

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use jiff::Timestamp;
use rusqlite::params;
use serde_json::json;

const SECONDS_PER_DAY: u64 = 60 * 60 * 24;
const TWENTY_YEARS_SECONDS: u64 = SECONDS_PER_DAY * 365 * 20;
const TEN_YEARS_SECONDS: u64 = SECONDS_PER_DAY * 365 * 10;
const NINETY_DAYS_SECONDS: u64 = SECONDS_PER_DAY * 90;

#[derive(Debug, Clone)]
pub struct SweepReport {
    pub started_at: Timestamp,
    pub completed_at: Timestamp,
    pub deleted_by_table: Vec<(String, u64)>,
    pub high_water_audit_occurred_at: Option<Timestamp>,
}

/// Nightly job — hard-deletes rows past their horizon and writes one `retention_sweep`
/// audit row recording counts and the high-water mark.
pub fn sweep(db: &Database, now: Timestamp) -> Result<SweepReport> {
    let started_at = now;

    let audit_cutoff = now
        .checked_sub(std::time::Duration::from_secs(TEN_YEARS_SECONDS))
        .map_err(|_| Error::Invariant("audit cutoff overflow"))?;
    let session_cutoff = now
        .checked_sub(std::time::Duration::from_secs(NINETY_DAYS_SECONDS))
        .map_err(|_| Error::Invariant("session cutoff overflow"))?;
    let user_cutoff = now
        .checked_sub(std::time::Duration::from_secs(TEN_YEARS_SECONDS))
        .map_err(|_| Error::Invariant("user cutoff overflow"))?;
    let suppression_cutoff = now
        .checked_sub(std::time::Duration::from_secs(TWENTY_YEARS_SECONDS))
        .map_err(|_| Error::Invariant("suppression cutoff overflow"))?;

    let report = db.with_writer(|conn| {
        let mut deleted_by_table: Vec<(String, u64)> = Vec::new();

        let high_water_str: Option<String> = conn
            .query_row(
                "SELECT MAX(occurred_at) FROM audit_log WHERE occurred_at < ?1",
                params![audit_cutoff.to_string()],
                |r| r.get(0),
            )
            .ok();
        let high_water_ts: Option<Timestamp> = high_water_str.and_then(|s| s.parse().ok());

        // Drop the trigger transiently; retention is the one legitimate deletion path.
        conn.execute("DROP TRIGGER trg_audit_log_no_delete", params![])?;
        let audit_deleted = conn.execute(
            "DELETE FROM audit_log WHERE occurred_at < ?1",
            params![audit_cutoff.to_string()],
        )?;
        conn.execute(
            "CREATE TRIGGER trg_audit_log_no_delete \
             BEFORE DELETE ON audit_log \
             BEGIN SELECT RAISE(ABORT, 'audit immutable'); END",
            params![],
        )?;
        deleted_by_table.push(("audit_log".into(), audit_deleted as u64));

        let session_deleted = conn.execute(
            "DELETE FROM auth_session WHERE refresh_expires_at < ?1",
            params![session_cutoff.to_string()],
        )?;
        deleted_by_table.push(("auth_session".into(), session_deleted as u64));

        let user_deleted = conn.execute(
            "DELETE FROM user WHERE disabled_at IS NOT NULL AND disabled_at < ?1",
            params![user_cutoff.to_string()],
        )?;
        deleted_by_table.push(("user".into(), user_deleted as u64));

        // Phase-1 approximation of "20-year clinical horizon": 20 years from suppressed_at.
        let patient_deleted = conn.execute(
            "DELETE FROM patient WHERE suppressed_at IS NOT NULL AND suppressed_at < ?1",
            params![suppression_cutoff.to_string()],
        )?;
        deleted_by_table.push(("patient".into(), patient_deleted as u64));

        let report_meta = json!({
            "deleted_by_table": deleted_by_table.iter().map(|(t, n)| json!({"table": t, "count": n})).collect::<Vec<_>>(),
            "high_water_audit_occurred_at": high_water_ts.map(|t: Timestamp| t.to_string()),
        });
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: None,
                auth_session_id: None,
                action: Action::RetentionSweep,
                target_type: "retention_sweep".into(),
                target_id: now.to_string(),
                patient_id: None,
                metadata: report_meta,
            },
        )?;

        Ok::<_, Error>(SweepReport {
            started_at,
            completed_at: db.clock().now(),
            deleted_by_table,
            high_water_audit_occurred_at: high_water_ts,
        })
    })?;
    Ok(report)
}
