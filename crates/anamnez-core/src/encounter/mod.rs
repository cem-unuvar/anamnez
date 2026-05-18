//! README §Data Modelling — `encounter` table + state machine.

use crate::audit::{self, Action, AppendInput};
use crate::code_systems::CodeSystem;
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{EncounterId, PatientId, UserId};
use crate::locking::Versioned;
use crate::patient_access::{caps, level_for_in_conn};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterKind {
    InPerson,
    Phone,
    Video,
    AsyncDocument,
}

impl EncounterKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InPerson => "in_person",
            Self::Phone => "phone",
            Self::Video => "video",
            Self::AsyncDocument => "async_document",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "in_person" => Ok(Self::InPerson),
            "phone" => Ok(Self::Phone),
            "video" => Ok(Self::Video),
            "async_document" => Ok(Self::AsyncDocument),
            _ => Err(Error::Invariant("unknown encounter kind")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterStatus {
    InProgress,
    Finished,
    Cancelled,
}

impl EncounterStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Finished => "finished",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "in_progress" => Ok(Self::InProgress),
            "finished" => Ok(Self::Finished),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(Error::Invariant("unknown encounter status")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encounter {
    pub id: EncounterId,
    pub patient_id: PatientId,
    pub provider_id: UserId,
    pub kind: EncounterKind,
    pub reason_text: String,
    pub reason_code: Option<String>,
    pub reason_code_system: Option<CodeSystem>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub status: EncounterStatus,
    pub created_at: Timestamp,
}

pub fn start(
    db: &Database,
    patient_id: PatientId,
    provider_id: UserId,
    kind: EncounterKind,
    reason_text: String,
) -> Result<Versioned<Encounter>> {
    let now = db.clock().now();
    let id = EncounterId::new();
    let encounter = Encounter {
        id,
        patient_id,
        provider_id,
        kind,
        reason_text: reason_text.clone(),
        reason_code: None,
        reason_code_system: None,
        started_at: now,
        ended_at: None,
        status: EncounterStatus::InProgress,
        created_at: now,
    };

    db.with_writer(|conn| {
        let lvl = level_for_in_conn(conn, provider_id, patient_id)?;
        match lvl {
            Some(l) if caps::can_write_clinical(l) => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }
        conn.execute(
            "INSERT INTO encounter \
             (id, patient_id, provider_id, kind, reason_text, started_at, status, created_at, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_progress', ?7, 1)",
            params![
                encounter.id.as_uuid().to_string(),
                encounter.patient_id.as_uuid().to_string(),
                encounter.provider_id.as_uuid().to_string(),
                encounter.kind.as_str(),
                encounter.reason_text,
                encounter.started_at.to_string(),
                encounter.created_at.to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(provider_id),
                auth_session_id: None,
                action: Action::EncounterStart,
                target_type: "encounter".into(),
                target_id: encounter.id.as_uuid().to_string(),
                patient_id: Some(encounter.patient_id),
                metadata: json!({"kind": encounter.kind.as_str()}),
            },
        )?;
        Ok(())
    })?;

    Ok(Versioned::new(encounter, 1))
}

pub fn finish(
    db: &Database,
    actor: UserId,
    id: EncounterId,
    expected_version: i64,
    reason_code: String,
    reason_code_system: CodeSystem,
) -> Result<Versioned<Encounter>> {
    if !reason_code_system.is_encounter_reason_scope() {
        return Err(Error::CodeSystemNotAllowed {
            code_system: reason_code_system.as_str().to_owned(),
            context: "encounter.reason_code_system",
        });
    }

    db.with_writer(|conn| {
        let current = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;
        // Access check.
        let lvl = level_for_in_conn(conn, actor, current.value.patient_id)?;
        match lvl {
            Some(l) if caps::can_write_clinical(l) => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }

        if current.version != expected_version {
            return Err(Error::Conflict {
                current_version: current.version,
                new_state_json: serde_json::to_string(&current.value)?,
            });
        }
        if !matches!(current.value.status, EncounterStatus::InProgress) {
            return Err(Error::InvalidStateTransition {
                from: match current.value.status {
                    EncounterStatus::InProgress => "in_progress",
                    EncounterStatus::Finished => "finished",
                    EncounterStatus::Cancelled => "cancelled",
                },
                to: "finished",
            });
        }

        let now = db.clock().now();
        let affected = conn.execute(
            "UPDATE encounter SET status = 'finished', reason_code = ?2, reason_code_system = ?3, ended_at = ?4, version = version + 1 \
             WHERE id = ?1 AND version = ?5",
            params![
                id.as_uuid().to_string(),
                reason_code,
                reason_code_system.as_str(),
                now.to_string(),
                expected_version,
            ],
        )?;
        if affected == 0 {
            let post = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;
            return Err(Error::Conflict {
                current_version: post.version,
                new_state_json: serde_json::to_string(&post.value)?,
            });
        }

        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::EncounterFinish,
                target_type: "encounter".into(),
                target_id: id.as_uuid().to_string(),
                patient_id: Some(current.value.patient_id),
                metadata: json!({
                    "reason_code": reason_code,
                    "reason_code_system": reason_code_system.as_str(),
                }),
            },
        )?;
        let mut next = current.value.clone();
        next.status = EncounterStatus::Finished;
        next.reason_code = Some(reason_code);
        next.reason_code_system = Some(reason_code_system);
        next.ended_at = Some(now);
        Ok(Versioned::new(next, expected_version + 1))
    })
}

pub fn cancel(
    db: &Database,
    actor: UserId,
    id: EncounterId,
    expected_version: i64,
) -> Result<Versioned<Encounter>> {
    db.with_writer(|conn| {
        let current = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;
        let lvl = level_for_in_conn(conn, actor, current.value.patient_id)?;
        match lvl {
            Some(l) if caps::can_write_clinical(l) => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }
        if current.version != expected_version {
            return Err(Error::Conflict {
                current_version: current.version,
                new_state_json: serde_json::to_string(&current.value)?,
            });
        }
        if !matches!(current.value.status, EncounterStatus::InProgress) {
            return Err(Error::InvalidStateTransition {
                from: match current.value.status {
                    EncounterStatus::InProgress => "in_progress",
                    EncounterStatus::Finished => "finished",
                    EncounterStatus::Cancelled => "cancelled",
                },
                to: "cancelled",
            });
        }
        let now = db.clock().now();
        let affected = conn.execute(
            "UPDATE encounter SET status = 'cancelled', ended_at = ?2, version = version + 1 \
             WHERE id = ?1 AND version = ?3",
            params![id.as_uuid().to_string(), now.to_string(), expected_version],
        )?;
        if affected == 0 {
            let post = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;
            return Err(Error::Conflict {
                current_version: post.version,
                new_state_json: serde_json::to_string(&post.value)?,
            });
        }
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::EncounterCancel,
                target_type: "encounter".into(),
                target_id: id.as_uuid().to_string(),
                patient_id: Some(current.value.patient_id),
                metadata: json!({}),
            },
        )?;
        let mut next = current.value.clone();
        next.status = EncounterStatus::Cancelled;
        next.ended_at = Some(now);
        Ok(Versioned::new(next, expected_version + 1))
    })
}

/// List all encounters on a patient. Caller must have any `patient_access` level.
/// Ordered newest-first by `started_at`.
pub fn list_by_patient(
    db: &Database,
    viewer: UserId,
    patient_id: PatientId,
) -> Result<Vec<Versioned<Encounter>>> {
    db.with_reader(|conn| {
        let lvl = level_for_in_conn(conn, viewer, patient_id)?;
        if lvl.is_none() {
            return Err(Error::NotFound);
        }
        let mut stmt = conn.prepare(
            "SELECT id FROM encounter WHERE patient_id = ?1 ORDER BY started_at DESC",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![patient_id.as_uuid().to_string()], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for s in ids {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("encounter.id not a UUID"))?;
            if let Some(v) = load_in_conn(conn, EncounterId(uuid))? {
                out.push(v);
            }
        }
        Ok(out)
    })
}

fn load_in_conn(
    conn: &rusqlite::Connection,
    id: EncounterId,
) -> Result<Option<Versioned<Encounter>>> {
    let row = conn
        .query_row(
            "SELECT id, patient_id, provider_id, kind, reason_text, reason_code, reason_code_system, \
                    started_at, ended_at, status, created_at, version \
             FROM encounter WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_encounter,
        )
        .optional()?;
    Ok(row)
}

fn row_to_encounter(row: &rusqlite::Row<'_>) -> rusqlite::Result<Versioned<Encounter>> {
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

    let id_str: String = row.get(0)?;
    let patient_id_str: String = row.get(1)?;
    let provider_id_str: String = row.get(2)?;
    let kind_str: String = row.get(3)?;
    let reason_text: String = row.get(4)?;
    let reason_code: Option<String> = row.get(5)?;
    let reason_code_system_str: Option<String> = row.get(6)?;
    let started_at_str: String = row.get(7)?;
    let ended_at_str: Option<String> = row.get(8)?;
    let status_str: String = row.get(9)?;
    let created_at_str: String = row.get(10)?;
    let version: i64 = row.get(11)?;

    let reason_code_system = match reason_code_system_str {
        None => None,
        Some(s) => Some(CodeSystem::parse_tag(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?),
    };

    let encounter = Encounter {
        id: EncounterId(parse_uuid(&id_str)?),
        patient_id: PatientId(parse_uuid(&patient_id_str)?),
        provider_id: UserId(parse_uuid(&provider_id_str)?),
        kind: EncounterKind::parse(&kind_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        reason_text,
        reason_code,
        reason_code_system,
        started_at: parse_ts(&started_at_str)?,
        ended_at: match ended_at_str {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        status: EncounterStatus::parse(&status_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        created_at: parse_ts(&created_at_str)?,
    };
    Ok(Versioned::new(encounter, version))
}
