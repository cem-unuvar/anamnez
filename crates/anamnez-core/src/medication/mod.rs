//! README §Data Modelling — `medication` table. `code_system` ∈ {ATC, TITCK}.

use crate::audit::{self, Action, AppendInput};
use crate::code_systems::{self, CodeSystem};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{EncounterId, MedicationId, PatientId, SourceDocumentId, UserId};
use crate::locking::Versioned;
use crate::patient_access::{caps, level_for_in_conn};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicationRoute {
    Oral,
    Iv,
    Im,
    Topical,
    Inhaled,
    Other,
}

impl MedicationRoute {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Oral => "oral",
            Self::Iv => "iv",
            Self::Im => "im",
            Self::Topical => "topical",
            Self::Inhaled => "inhaled",
            Self::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "oral" => Ok(Self::Oral),
            "iv" => Ok(Self::Iv),
            "im" => Ok(Self::Im),
            "topical" => Ok(Self::Topical),
            "inhaled" => Ok(Self::Inhaled),
            "other" => Ok(Self::Other),
            _ => Err(Error::Invariant("unknown medication route")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicationStatus {
    Active,
    Completed,
    Stopped,
    EnteredInError,
}

impl MedicationStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::EnteredInError => "entered_in_error",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "active" => Ok(Self::Active),
            "completed" => Ok(Self::Completed),
            "stopped" => Ok(Self::Stopped),
            "entered_in_error" => Ok(Self::EnteredInError),
            _ => Err(Error::Invariant("unknown medication status")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medication {
    pub id: MedicationId,
    pub patient_id: PatientId,
    pub code: String,
    pub code_system: CodeSystem,
    pub display_text: String,
    pub dose_quantity: Option<f64>,
    pub dose_unit: Option<String>,
    pub frequency_text: Option<String>,
    pub route: MedicationRoute,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub reason_text: Option<String>,
    pub status: MedicationStatus,
    pub prescriber_id: Option<UserId>,
    pub recorded_at: Timestamp,
    pub recorded_by: UserId,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMedication {
    pub patient_id: PatientId,
    pub code: String,
    pub code_system: CodeSystem,
    pub display_text: String,
    pub dose_quantity: Option<f64>,
    pub dose_unit: Option<String>,
    pub frequency_text: Option<String>,
    pub route: MedicationRoute,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub reason_text: Option<String>,
    pub status: MedicationStatus,
    pub prescriber_id: Option<UserId>,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

/// Validation:
/// - `code_system` must be `Atc` or `Titck` (DB also enforces).
/// - `(code_system, code)` pair must exist in the relevant lookup table.
/// - Caller must have collaborator-or-better access to the patient.
pub fn create(db: &Database, actor: UserId, input: NewMedication) -> Result<Versioned<Medication>> {
    if !input.code_system.is_medication_scope() {
        return Err(Error::CodeSystemNotAllowed {
            code_system: input.code_system.as_str().to_owned(),
            context: "medication",
        });
    }

    let id = MedicationId::new();
    let now = db.clock().now();

    let medication = Medication {
        id,
        patient_id: input.patient_id,
        code: input.code.clone(),
        code_system: input.code_system,
        display_text: input.display_text.clone(),
        dose_quantity: input.dose_quantity,
        dose_unit: input.dose_unit.clone(),
        frequency_text: input.frequency_text.clone(),
        route: input.route,
        started_at: input.started_at,
        ended_at: input.ended_at,
        reason_text: input.reason_text.clone(),
        status: input.status,
        prescriber_id: input.prescriber_id,
        recorded_at: now,
        recorded_by: actor,
        source_id: input.source_id,
        encounter_id: input.encounter_id,
    };

    db.with_writer(|conn| {
        let lvl = level_for_in_conn(conn, actor, input.patient_id)?;
        match lvl {
            Some(l) if caps::can_write_clinical(l) => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }

        code_systems::lookup_in_conn(conn, medication.code_system, &medication.code)?;

        conn.execute(
            "INSERT INTO medication \
             (id, patient_id, code, code_system, display_text, dose_quantity, dose_unit, frequency_text, \
              route, started_at, ended_at, reason_text, status, prescriber_id, recorded_at, recorded_by, \
              source_id, encounter_id, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 1)",
            params![
                medication.id.as_uuid().to_string(),
                medication.patient_id.as_uuid().to_string(),
                medication.code,
                medication.code_system.as_str(),
                medication.display_text,
                medication.dose_quantity,
                medication.dose_unit,
                medication.frequency_text,
                medication.route.as_str(),
                medication.started_at.to_string(),
                medication.ended_at.map(|t| t.to_string()),
                medication.reason_text,
                medication.status.as_str(),
                medication.prescriber_id.map(|i| i.as_uuid().to_string()),
                medication.recorded_at.to_string(),
                medication.recorded_by.as_uuid().to_string(),
                medication.source_id.map(|i| i.as_uuid().to_string()),
                medication.encounter_id.map(|i| i.as_uuid().to_string()),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::MedicationCreate,
                target_type: "medication".into(),
                target_id: medication.id.as_uuid().to_string(),
                patient_id: Some(medication.patient_id),
                metadata: json!({"status": medication.status.as_str(), "route": medication.route.as_str()}),
            },
        )?;
        Ok(())
    })?;

    Ok(Versioned::new(medication, 1))
}

pub fn amend(
    db: &Database,
    actor: UserId,
    id: MedicationId,
    expected_version: i64,
    patch: MedicationPatch,
) -> Result<Versioned<Medication>> {
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

        let mut next = current.value.clone();
        if let Some(v) = patch.dose_quantity {
            next.dose_quantity = v;
        }
        if let Some(v) = patch.dose_unit {
            next.dose_unit = v;
        }
        if let Some(v) = patch.frequency_text {
            next.frequency_text = v;
        }
        if let Some(v) = patch.ended_at {
            next.ended_at = v;
        }
        if let Some(v) = patch.status {
            next.status = v;
        }
        next.recorded_at = db.clock().now();

        let affected = conn.execute(
            "UPDATE medication SET \
             dose_quantity = ?2, dose_unit = ?3, frequency_text = ?4, ended_at = ?5, status = ?6, \
             recorded_at = ?7, version = version + 1 \
             WHERE id = ?1 AND version = ?8",
            params![
                next.id.as_uuid().to_string(),
                next.dose_quantity,
                next.dose_unit,
                next.frequency_text,
                next.ended_at.map(|t| t.to_string()),
                next.status.as_str(),
                next.recorded_at.to_string(),
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
            db.clock().now(),
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::MedicationAmend,
                target_type: "medication".into(),
                target_id: next.id.as_uuid().to_string(),
                patient_id: Some(next.patient_id),
                metadata: json!({"new_version": expected_version + 1}),
            },
        )?;

        Ok(Versioned::new(next, expected_version + 1))
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MedicationPatch {
    pub dose_quantity: Option<Option<f64>>,
    pub dose_unit: Option<Option<String>>,
    pub frequency_text: Option<Option<String>>,
    pub ended_at: Option<Option<Timestamp>>,
    pub status: Option<MedicationStatus>,
}

/// List all medications on a patient. Caller must have any `patient_access` level.
/// Ordered newest-first by `recorded_at`.
pub fn list_by_patient(
    db: &Database,
    viewer: UserId,
    patient_id: PatientId,
) -> Result<Vec<Versioned<Medication>>> {
    db.with_reader(|conn| {
        let lvl = level_for_in_conn(conn, viewer, patient_id)?;
        if lvl.is_none() {
            return Err(Error::NotFound);
        }
        let mut stmt = conn.prepare(
            "SELECT id FROM medication WHERE patient_id = ?1 ORDER BY recorded_at DESC",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![patient_id.as_uuid().to_string()], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for s in ids {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("medication.id not a UUID"))?;
            if let Some(v) = load_in_conn(conn, MedicationId(uuid))? {
                out.push(v);
            }
        }
        Ok(out)
    })
}

fn load_in_conn(
    conn: &rusqlite::Connection,
    id: MedicationId,
) -> Result<Option<Versioned<Medication>>> {
    let row = conn
        .query_row(
            "SELECT id, patient_id, code, code_system, display_text, dose_quantity, dose_unit, \
                    frequency_text, route, started_at, ended_at, reason_text, status, prescriber_id, \
                    recorded_at, recorded_by, source_id, encounter_id, version \
             FROM medication WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_medication,
        )
        .optional()?;
    Ok(row)
}

fn row_to_medication(row: &rusqlite::Row<'_>) -> rusqlite::Result<Versioned<Medication>> {
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
    let code: String = row.get(2)?;
    let code_system_str: String = row.get(3)?;
    let display_text: String = row.get(4)?;
    let dose_quantity: Option<f64> = row.get(5)?;
    let dose_unit: Option<String> = row.get(6)?;
    let frequency_text: Option<String> = row.get(7)?;
    let route_str: String = row.get(8)?;
    let started_at_str: String = row.get(9)?;
    let ended_at_str: Option<String> = row.get(10)?;
    let reason_text: Option<String> = row.get(11)?;
    let status_str: String = row.get(12)?;
    let prescriber_id_str: Option<String> = row.get(13)?;
    let recorded_at_str: String = row.get(14)?;
    let recorded_by_str: String = row.get(15)?;
    let source_id_str: Option<String> = row.get(16)?;
    let encounter_id_str: Option<String> = row.get(17)?;
    let version: i64 = row.get(18)?;

    let medication = Medication {
        id: MedicationId(parse_uuid(&id_str)?),
        patient_id: PatientId(parse_uuid(&patient_id_str)?),
        code,
        code_system: CodeSystem::parse_tag(&code_system_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        display_text,
        dose_quantity,
        dose_unit,
        frequency_text,
        route: MedicationRoute::parse(&route_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        started_at: parse_ts(&started_at_str)?,
        ended_at: match ended_at_str {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        reason_text,
        status: MedicationStatus::parse(&status_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        prescriber_id: match prescriber_id_str {
            None => None,
            Some(s) => Some(UserId(parse_uuid(&s)?)),
        },
        recorded_at: parse_ts(&recorded_at_str)?,
        recorded_by: UserId(parse_uuid(&recorded_by_str)?),
        source_id: match source_id_str {
            None => None,
            Some(s) => Some(SourceDocumentId(parse_uuid(&s)?)),
        },
        encounter_id: match encounter_id_str {
            None => None,
            Some(s) => Some(EncounterId(parse_uuid(&s)?)),
        },
    };

    Ok(Versioned::new(medication, version))
}
