//! SPEC §Deployment — `workstation` row CRUD + revocation deny-set load.
//!
//! The daemon loads `list_revoked()` into an in-memory `HashSet<WorkstationId>` at boot
//! and rejects mTLS handshakes whose client cert maps to a revoked device.

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{AuthSessionId, UserId, WorkstationId};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Bound,
    Shared,
}

impl Mode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Shared => "shared",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "bound" => Ok(Self::Bound),
            "shared" => Ok(Self::Shared),
            _ => Err(Error::Invariant("unknown workstation mode")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workstation {
    pub id: WorkstationId,
    pub label: String,
    pub mode: Mode,
    pub bound_user_id: Option<UserId>,
    pub cert_serial: String,
    pub cert_fingerprint: String,
    pub enrolled_at: Timestamp,
    pub enrolled_by: UserId,
    pub last_seen_at: Option<Timestamp>,
    pub revoked_at: Option<Timestamp>,
    pub revoked_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewWorkstation {
    pub label: String,
    pub mode: Mode,
    pub bound_user_id: Option<UserId>,
    pub cert_serial: String,
    pub cert_fingerprint: String,
}

/// Enroll a new workstation. `mode = Bound` requires `bound_user_id`; `Shared` forbids it.
/// Audits `Action::WorkstationEnroll`.
pub fn enroll(db: &Database, admin: UserId, input: NewWorkstation) -> Result<Workstation> {
    if matches!(input.mode, Mode::Bound) && input.bound_user_id.is_none() {
        return Err(Error::Invariant(
            "workstation mode=bound requires bound_user_id",
        ));
    }
    if matches!(input.mode, Mode::Shared) && input.bound_user_id.is_some() {
        return Err(Error::Invariant(
            "workstation mode=shared forbids bound_user_id",
        ));
    }
    let id = WorkstationId::new();
    let now = db.clock().now();
    let ws = Workstation {
        id,
        label: input.label.clone(),
        mode: input.mode,
        bound_user_id: input.bound_user_id,
        cert_serial: input.cert_serial.clone(),
        cert_fingerprint: input.cert_fingerprint.clone(),
        enrolled_at: now,
        enrolled_by: admin,
        last_seen_at: None,
        revoked_at: None,
        revoked_reason: None,
    };

    db.with_writer(|conn| {
        conn.execute(
            "INSERT INTO workstation \
             (id, label, mode, bound_user_id, cert_serial, cert_fingerprint, enrolled_at, enrolled_by) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                ws.id.as_uuid().to_string(),
                ws.label,
                ws.mode.as_str(),
                ws.bound_user_id.map(|u| u.as_uuid().to_string()),
                ws.cert_serial,
                ws.cert_fingerprint,
                ws.enrolled_at.to_string(),
                ws.enrolled_by.as_uuid().to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::WorkstationEnroll,
                target_type: "workstation".into(),
                target_id: ws.id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"label": ws.label, "mode": ws.mode.as_str()}),
            },
        )?;
        Ok(())
    })?;
    Ok(ws)
}

/// Revoke a workstation. Sets `revoked_at`; the daemon adds the device_id to its
/// in-memory deny set so subsequent mTLS handshakes fail. Audits `Action::WorkstationRevoke`.
pub fn revoke(db: &Database, admin: UserId, id: WorkstationId, reason: String) -> Result<()> {
    let now = db.clock().now();
    db.with_writer(|conn| {
        let affected = conn.execute(
            "UPDATE workstation SET revoked_at = ?2, revoked_reason = ?3 \
             WHERE id = ?1 AND revoked_at IS NULL",
            params![id.as_uuid().to_string(), now.to_string(), reason],
        )?;
        if affected == 0 {
            return Err(Error::NotFound);
        }
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin),
                auth_session_id: None,
                action: Action::WorkstationRevoke,
                target_type: "workstation".into(),
                target_id: id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"reason": reason}),
            },
        )?;
        Ok(())
    })
}

/// Snapshot of revoked device_ids — loaded into the daemon's in-memory deny set at boot.
pub fn list_revoked(db: &Database) -> Result<Vec<WorkstationId>> {
    db.with_reader(|conn| {
        let mut stmt = conn.prepare("SELECT id FROM workstation WHERE revoked_at IS NOT NULL")?;
        let rows: Vec<String> = stmt
            .query_map(params![], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for s in rows {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("workstation.id not a UUID"))?;
            out.push(WorkstationId(uuid));
        }
        Ok(out)
    })
}

pub fn get(db: &Database, id: WorkstationId) -> Result<Option<Workstation>> {
    db.with_reader(|conn| {
        conn.query_row(
            "SELECT id, label, mode, bound_user_id, cert_serial, cert_fingerprint, \
                    enrolled_at, enrolled_by, last_seen_at, revoked_at, revoked_reason \
             FROM workstation WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_workstation,
        )
        .optional()
        .map_err(Error::from)
    })
}

/// Look up `auth_session.id`s for live sessions bound to a workstation — used to
/// fan out `ForcedLogout` SSE events after a revocation.
pub fn list_sessions_on(db: &Database, id: WorkstationId) -> Result<Vec<AuthSessionId>> {
    db.with_reader(|conn| {
        let mut stmt = conn
            .prepare("SELECT id FROM auth_session WHERE device_id = ?1 AND revoked_at IS NULL")?;
        let rows: Vec<String> = stmt
            .query_map(params![id.as_uuid().to_string()], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for s in rows {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("auth_session.id not a UUID"))?;
            out.push(AuthSessionId(uuid));
        }
        Ok(out)
    })
}

fn row_to_workstation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workstation> {
    let parse_uuid = |s: &str| {
        uuid::Uuid::parse_str(s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    let parse_ts = |s: &str| -> rusqlite::Result<Timestamp> {
        s.parse().map_err(|e: jiff::Error| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };

    let id: String = row.get(0)?;
    let label: String = row.get(1)?;
    let mode: String = row.get(2)?;
    let bound_user_id: Option<String> = row.get(3)?;
    let cert_serial: String = row.get(4)?;
    let cert_fingerprint: String = row.get(5)?;
    let enrolled_at: String = row.get(6)?;
    let enrolled_by: String = row.get(7)?;
    let last_seen_at: Option<String> = row.get(8)?;
    let revoked_at: Option<String> = row.get(9)?;
    let revoked_reason: Option<String> = row.get(10)?;

    Ok(Workstation {
        id: WorkstationId(parse_uuid(&id)?),
        label,
        mode: Mode::parse(&mode).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        bound_user_id: match bound_user_id {
            None => None,
            Some(s) => Some(UserId(parse_uuid(&s)?)),
        },
        cert_serial,
        cert_fingerprint,
        enrolled_at: parse_ts(&enrolled_at)?,
        enrolled_by: UserId(parse_uuid(&enrolled_by)?),
        last_seen_at: match last_seen_at {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        revoked_at: match revoked_at {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        revoked_reason,
    })
}
