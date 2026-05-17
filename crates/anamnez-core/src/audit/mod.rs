//! README §Storage → Audit log integrity — append-only, tamper-evident.
//!
//! `audit::append` is the only function that writes the `audit_log` table.
//! Enforcement is layered: no other code path inserts; a `BEFORE UPDATE/DELETE`
//! trigger aborts mutation; row hashes form a chain verified on startup.

pub mod action;
pub mod canonical;
pub mod hash;
pub mod verify;

pub use action::Action;

use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{AuditLogId, AuthSessionId, PatientId, UserId};
use jiff::Timestamp;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogRow {
    pub id: AuditLogId,
    pub occurred_at: Timestamp,
    pub actor_user_id: Option<UserId>,
    pub auth_session_id: Option<AuthSessionId>,
    pub action: Action,
    pub target_type: String,
    pub target_id: String,
    pub patient_id: Option<PatientId>,
    pub metadata: JsonValue,
    pub prev_hash: Vec<u8>,
    pub row_hash: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AppendInput {
    pub actor_user_id: Option<UserId>,
    pub auth_session_id: Option<AuthSessionId>,
    pub action: Action,
    pub target_type: String,
    pub target_id: String,
    pub patient_id: Option<PatientId>,
    pub metadata: JsonValue,
}

/// Append a single audit row. The only public write path into `audit_log`.
///
/// Single-writer-mutex serializes appends. Inside the writer lock we:
/// 1. Read the previous head's `id` and `row_hash` (genesis = all-zero, id = 0).
/// 2. Compute the new row's hash.
/// 3. Insert with the pre-computed `id` so the hash matches what gets stored.
pub fn append(db: &Database, input: AppendInput) -> Result<AuditLogId> {
    db.with_writer(|conn| append_in_conn(conn, db.clock().now(), input))
}

/// Variant for callers that already hold a writer connection (e.g. multi-step
/// transactions in subsystem G's clinical writes). Accepts `&Connection` so a
/// `rusqlite::Transaction` (which derefs to `&Connection`) can be passed
/// through directly — bundle apply lives inside one outer transaction and
/// needs to append from within it.
pub fn append_in_conn(
    conn: &Connection,
    occurred_at: Timestamp,
    input: AppendInput,
) -> Result<AuditLogId> {
    let (prev_id, prev_hash): (i64, Vec<u8>) = conn
        .query_row(
            "SELECT COALESCE(MAX(id), 0), \
             COALESCE((SELECT row_hash FROM audit_log ORDER BY id DESC LIMIT 1), ZEROBLOB(32)) \
             FROM audit_log",
            params![],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .map_err(Error::from)?;

    let new_id = AuditLogId(prev_id + 1);
    let row_hash = hash::compute(
        &prev_hash,
        new_id,
        occurred_at,
        input.actor_user_id,
        input.auth_session_id,
        input.action,
        &input.target_type,
        &input.target_id,
        input.patient_id,
        &input.metadata,
    );

    let metadata_str = serde_json::to_string(&input.metadata)?;
    let actor_str = input.actor_user_id.map(|u| u.as_uuid().to_string());
    let session_str = input.auth_session_id.map(|s| s.as_uuid().to_string());
    let patient_str = input.patient_id.map(|p| p.as_uuid().to_string());

    conn.execute(
        "INSERT INTO audit_log \
         (id, occurred_at, actor_user_id, auth_session_id, action, target_type, target_id, patient_id, metadata, prev_hash, row_hash) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            new_id.as_i64(),
            occurred_at.to_string(),
            actor_str,
            session_str,
            input.action.as_str(),
            input.target_type,
            input.target_id,
            patient_str,
            metadata_str,
            prev_hash,
            row_hash.to_vec(),
        ],
    )?;

    Ok(new_id)
}

/// Run breach-scope analysis. README §Compliance → KVKK-derived features.
pub fn breach_report(db: &Database, scope: BreachScope) -> Result<Vec<BreachReportRow>> {
    match scope {
        BreachScope::BySession(sid) => db.with_reader(|conn| {
            let mut stmt = conn.prepare(
                "SELECT occurred_at, action, patient_id, target_type, target_id \
                 FROM audit_log WHERE auth_session_id = ?1 ORDER BY occurred_at ASC",
            )?;
            let rows = stmt.query_map(params![sid.as_uuid().to_string()], breach_row_from)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        }),
        BreachScope::ByUser {
            user_id,
            from,
            until,
        } => db.with_reader(|conn| {
            let mut stmt = conn.prepare(
                "SELECT occurred_at, action, patient_id, target_type, target_id \
                 FROM audit_log WHERE actor_user_id = ?1 AND occurred_at >= ?2 AND occurred_at <= ?3 \
                 ORDER BY occurred_at ASC",
            )?;
            let rows = stmt.query_map(
                params![
                    user_id.as_uuid().to_string(),
                    from.to_string(),
                    until.to_string()
                ],
                breach_row_from,
            )?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        }),
    }
}

fn breach_row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<BreachReportRow> {
    let occurred_at_str: String = row.get(0)?;
    let action_str: String = row.get(1)?;
    let patient_id_str: Option<String> = row.get(2)?;
    let target_type: String = row.get(3)?;
    let target_id: String = row.get(4)?;
    let occurred_at: Timestamp = occurred_at_str.parse().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let action: Action = serde_json::from_str(&format!("\"{action_str}\"")).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let patient_id = match patient_id_str {
        None => None,
        Some(s) => {
            let uuid = uuid::Uuid::parse_str(&s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Some(PatientId(uuid))
        }
    };
    Ok(BreachReportRow {
        occurred_at,
        action,
        patient_id,
        target_type,
        target_id,
    })
}

#[derive(Debug, Clone)]
pub enum BreachScope {
    BySession(AuthSessionId),
    ByUser {
        user_id: UserId,
        from: Timestamp,
        until: Timestamp,
    },
}

#[derive(Debug, Clone)]
pub struct BreachReportRow {
    pub occurred_at: Timestamp,
    pub action: Action,
    pub patient_id: Option<PatientId>,
    pub target_type: String,
    pub target_id: String,
}
