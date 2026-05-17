//! README §Data Modelling — `observation` as a first-class clinical row.
//!
//! Amendments are in-place version bumps (no supersession chain). `final` requires
//! `(code, code_system)`; `preliminary` allows null `code` with `display_text` preserved.

use crate::audit::{self, Action, AppendInput};
use crate::code_systems::{self, CodeSystem};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{EncounterId, ObservationId, PatientId, SourceDocumentId, UserId};
use crate::locking::Versioned;
use crate::patient_access::{caps, level_for_in_conn};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    Preliminary,
    Final,
    Amended,
}

impl ObservationStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preliminary => "preliminary",
            Self::Final => "final",
            Self::Amended => "amended",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "preliminary" => Ok(Self::Preliminary),
            "final" => Ok(Self::Final),
            "amended" => Ok(Self::Amended),
            _ => Err(Error::Invariant("unknown observation status")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractedBy {
    Manual,
    Llm,
}

impl ExtractedBy {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Llm => "llm",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "manual" => Ok(Self::Manual),
            "llm" => Ok(Self::Llm),
            _ => Err(Error::Invariant("unknown extracted_by")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueQuantity {
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObservationValue {
    Quantity(ValueQuantity),
    String(String),
    Codeable {
        code_system: CodeSystem,
        code: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub patient_id: PatientId,
    pub recorded_at: Timestamp,
    pub effective_period_start: Timestamp,
    pub effective_period_end: Option<Timestamp>,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub value: Option<ObservationValue>,
    pub status: ObservationStatus,
    pub is_problem_list_item: bool,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
    pub extracted_by: ExtractedBy,
    pub model_version: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewObservation {
    pub patient_id: PatientId,
    pub effective_period_start: Timestamp,
    pub effective_period_end: Option<Timestamp>,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub value: Option<ObservationValue>,
    pub status: ObservationStatus,
    pub is_problem_list_item: bool,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
    pub extracted_by: ExtractedBy,
    pub model_version: Option<String>,
    pub confidence: Option<f64>,
}

/// Validation rules:
/// - `code_system` must be in the observation-scoped subset (`SKRS-VP` rejected).
/// - `status = final` requires `code` and `code_system`.
/// - If both code and code_system are set, the pair must exist in lookup tables.
/// - Caller must have collaborator-or-better access to the patient.
pub fn create(
    db: &Database,
    actor: UserId,
    input: NewObservation,
) -> Result<Versioned<Observation>> {
    if let Some(cs) = input.code_system {
        if !cs.is_observation_scope() {
            return Err(Error::CodeSystemNotAllowed {
                code_system: cs.as_str().to_owned(),
                context: "observation",
            });
        }
    }
    if matches!(input.status, ObservationStatus::Final)
        && (input.code.is_none() || input.code_system.is_none())
    {
        return Err(Error::Invariant(
            "status=final requires code and code_system",
        ));
    }

    let id = ObservationId::new();
    let now = db.clock().now();

    let observation = Observation {
        id,
        patient_id: input.patient_id,
        recorded_at: now,
        effective_period_start: input.effective_period_start,
        effective_period_end: input.effective_period_end,
        code: input.code.clone(),
        code_system: input.code_system,
        display_text: input.display_text.clone(),
        value: input.value.clone(),
        status: input.status,
        is_problem_list_item: input.is_problem_list_item,
        source_id: input.source_id,
        encounter_id: input.encounter_id,
        extracted_by: input.extracted_by,
        model_version: input.model_version.clone(),
        confidence: input.confidence,
    };

    db.with_writer(|conn| {
        // Access check.
        let lvl = level_for_in_conn(conn, actor, input.patient_id)?;
        match lvl {
            Some(l) if caps::can_write_clinical(l) => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }

        // (code_system, code) validation against lookup tables when both are set.
        if let (Some(cs), Some(code)) = (input.code_system, input.code.as_deref()) {
            code_systems::lookup_in_conn(conn, cs, code)?;
        }

        let (vq_val, vq_unit, v_str, vc_sys, vc_code) = unpack_value(observation.value.as_ref());

        conn.execute(
            "INSERT INTO observation \
             (id, patient_id, recorded_at, effective_period_start, effective_period_end, code, code_system, \
              display_text, value_quantity_value, value_quantity_unit, value_string, value_codeable_system, \
              value_codeable_code, status, is_problem_list_item, source_id, encounter_id, extracted_by, \
              model_version, confidence, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, 1)",
            params![
                observation.id.as_uuid().to_string(),
                observation.patient_id.as_uuid().to_string(),
                observation.recorded_at.to_string(),
                observation.effective_period_start.to_string(),
                observation.effective_period_end.map(|t| t.to_string()),
                observation.code,
                observation.code_system.map(|c| c.as_str()),
                observation.display_text,
                vq_val,
                vq_unit,
                v_str,
                vc_sys,
                vc_code,
                observation.status.as_str(),
                i64::from(observation.is_problem_list_item),
                observation.source_id.map(|i| i.as_uuid().to_string()),
                observation.encounter_id.map(|i| i.as_uuid().to_string()),
                observation.extracted_by.as_str(),
                observation.model_version,
                observation.confidence,
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::ObservationCreate,
                target_type: "observation".into(),
                target_id: observation.id.as_uuid().to_string(),
                patient_id: Some(observation.patient_id),
                metadata: json!({"status": observation.status.as_str()}),
            },
        )?;
        Ok(())
    })?;

    Ok(Versioned::new(observation, 1))
}

/// In-place amendment: bumps `version`, sets `status = amended`. Optimistic locking
/// on `expected_version`. Stale version → `Error::Conflict { new_state_json }`.
pub fn amend(
    db: &Database,
    actor: UserId,
    id: ObservationId,
    expected_version: i64,
    patch: ObservationPatch,
) -> Result<Versioned<Observation>> {
    db.with_writer(|conn| {
        // Load current row.
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

        // Apply patch onto current.
        let mut next = current.value.clone();
        if let Some(v) = patch.effective_period_end {
            next.effective_period_end = v;
        }
        if let Some(v) = patch.code {
            next.code = v;
        }
        if let Some(v) = patch.code_system {
            next.code_system = v;
        }
        if let Some(v) = patch.display_text {
            next.display_text = v;
        }
        if let Some(v) = patch.value {
            next.value = v;
        }
        if let Some(v) = patch.status {
            next.status = v;
        }
        if let Some(v) = patch.is_problem_list_item {
            next.is_problem_list_item = v;
        }
        // README simplicity rule: amendments set status to 'amended'.
        next.status = ObservationStatus::Amended;
        next.recorded_at = db.clock().now();

        if let Some(cs) = next.code_system {
            if !cs.is_observation_scope() {
                return Err(Error::CodeSystemNotAllowed {
                    code_system: cs.as_str().to_owned(),
                    context: "observation",
                });
            }
        }
        if let (Some(cs), Some(code)) = (next.code_system, next.code.as_deref()) {
            code_systems::lookup_in_conn(conn, cs, code)?;
        }

        let (vq_val, vq_unit, v_str, vc_sys, vc_code) = unpack_value(next.value.as_ref());

        let affected = conn.execute(
            "UPDATE observation SET \
             recorded_at = ?2, effective_period_end = ?3, code = ?4, code_system = ?5, \
             display_text = ?6, value_quantity_value = ?7, value_quantity_unit = ?8, value_string = ?9, \
             value_codeable_system = ?10, value_codeable_code = ?11, status = ?12, is_problem_list_item = ?13, \
             version = version + 1 \
             WHERE id = ?1 AND version = ?14",
            params![
                next.id.as_uuid().to_string(),
                next.recorded_at.to_string(),
                next.effective_period_end.map(|t| t.to_string()),
                next.code,
                next.code_system.map(|c| c.as_str()),
                next.display_text,
                vq_val,
                vq_unit,
                v_str,
                vc_sys,
                vc_code,
                next.status.as_str(),
                i64::from(next.is_problem_list_item),
                expected_version,
            ],
        )?;
        if affected == 0 {
            // Concurrent edit slipped in between load and update.
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
                action: Action::ObservationAmend,
                target_type: "observation".into(),
                target_id: next.id.as_uuid().to_string(),
                patient_id: Some(next.patient_id),
                metadata: json!({"new_version": expected_version + 1}),
            },
        )?;

        Ok(Versioned::new(next, expected_version + 1))
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservationPatch {
    pub effective_period_end: Option<Option<Timestamp>>,
    pub code: Option<Option<String>>,
    pub code_system: Option<Option<CodeSystem>>,
    pub display_text: Option<String>,
    pub value: Option<Option<ObservationValue>>,
    pub status: Option<ObservationStatus>,
    pub is_problem_list_item: Option<bool>,
}

/// README §Data Modelling — "The active problem list is a query for these rows where
/// `effective_period_end IS NULL` and `status = 'final'`."
pub fn problem_list(
    db: &Database,
    viewer: UserId,
    patient_id: PatientId,
) -> Result<Vec<Versioned<Observation>>> {
    db.with_reader(|conn| {
        let lvl = level_for_in_conn(conn, viewer, patient_id)?;
        if lvl.is_none() {
            return Err(Error::NotFound);
        }
        let mut stmt = conn.prepare(
            "SELECT id FROM observation \
             WHERE patient_id = ?1 AND is_problem_list_item = 1 \
               AND status = 'final' AND code_system = 'ICD10TM' \
               AND effective_period_end IS NULL \
             ORDER BY recorded_at DESC",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![patient_id.as_uuid().to_string()], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for s in ids {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("observation.id not a UUID"))?;
            if let Some(v) = load_in_conn(conn, ObservationId(uuid))? {
                out.push(v);
            }
        }
        Ok(out)
    })
}

/// Read a single observation row. Requires any `patient_access` level on the
/// observation's patient.
pub fn get(db: &Database, viewer: UserId, id: ObservationId) -> Result<Versioned<Observation>> {
    db.with_reader(|conn| {
        let obs = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;
        let lvl = level_for_in_conn(conn, viewer, obs.value.patient_id)?;
        match lvl {
            Some(l) if caps::can_read(l) => Ok(obs),
            _ => Err(Error::NotFound),
        }
    })
}

pub fn list_by_patient(
    db: &Database,
    viewer: UserId,
    patient_id: PatientId,
) -> Result<Vec<Versioned<Observation>>> {
    db.with_reader(|conn| {
        let lvl = level_for_in_conn(conn, viewer, patient_id)?;
        if lvl.is_none() {
            return Err(Error::NotFound);
        }
        let mut stmt = conn.prepare(
            "SELECT id FROM observation WHERE patient_id = ?1 ORDER BY recorded_at DESC",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![patient_id.as_uuid().to_string()], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for s in ids {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| Error::Invariant("observation.id not a UUID"))?;
            if let Some(v) = load_in_conn(conn, ObservationId(uuid))? {
                out.push(v);
            }
        }
        Ok(out)
    })
}

fn load_in_conn(
    conn: &rusqlite::Connection,
    id: ObservationId,
) -> Result<Option<Versioned<Observation>>> {
    let row = conn
        .query_row(
            "SELECT id, patient_id, recorded_at, effective_period_start, effective_period_end, code, code_system, \
                    display_text, value_quantity_value, value_quantity_unit, value_string, value_codeable_system, \
                    value_codeable_code, status, is_problem_list_item, source_id, encounter_id, extracted_by, \
                    model_version, confidence, version \
             FROM observation WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_observation,
        )
        .optional()?;
    Ok(row)
}

fn row_to_observation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Versioned<Observation>> {
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
    let recorded_at_str: String = row.get(2)?;
    let effective_start_str: String = row.get(3)?;
    let effective_end_str: Option<String> = row.get(4)?;
    let code: Option<String> = row.get(5)?;
    let code_system_str: Option<String> = row.get(6)?;
    let display_text: String = row.get(7)?;
    let vq_val: Option<f64> = row.get(8)?;
    let vq_unit: Option<String> = row.get(9)?;
    let v_str: Option<String> = row.get(10)?;
    let vc_sys: Option<String> = row.get(11)?;
    let vc_code: Option<String> = row.get(12)?;
    let status_str: String = row.get(13)?;
    let is_problem: i64 = row.get(14)?;
    let source_id_str: Option<String> = row.get(15)?;
    let encounter_id_str: Option<String> = row.get(16)?;
    let extracted_by_str: String = row.get(17)?;
    let model_version: Option<String> = row.get(18)?;
    let confidence: Option<f64> = row.get(19)?;
    let version: i64 = row.get(20)?;

    let value = if let (Some(v), Some(u)) = (vq_val, vq_unit.as_deref()) {
        Some(ObservationValue::Quantity(ValueQuantity {
            value: v,
            unit: u.to_owned(),
        }))
    } else if let Some(s) = v_str {
        Some(ObservationValue::String(s))
    } else if let (Some(sys), Some(c)) = (vc_sys.as_deref(), vc_code.as_deref()) {
        let cs = CodeSystem::parse_tag(sys).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Some(ObservationValue::Codeable {
            code_system: cs,
            code: c.to_owned(),
        })
    } else {
        None
    };

    let code_system = match code_system_str {
        None => None,
        Some(s) => Some(CodeSystem::parse_tag(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?),
    };

    let observation = Observation {
        id: ObservationId(parse_uuid(&id_str)?),
        patient_id: PatientId(parse_uuid(&patient_id_str)?),
        recorded_at: parse_ts(&recorded_at_str)?,
        effective_period_start: parse_ts(&effective_start_str)?,
        effective_period_end: match effective_end_str {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        code,
        code_system,
        display_text,
        value,
        status: ObservationStatus::parse(&status_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        is_problem_list_item: is_problem != 0,
        source_id: match source_id_str {
            None => None,
            Some(s) => Some(SourceDocumentId(parse_uuid(&s)?)),
        },
        encounter_id: match encounter_id_str {
            None => None,
            Some(s) => Some(EncounterId(parse_uuid(&s)?)),
        },
        extracted_by: ExtractedBy::parse(&extracted_by_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        model_version,
        confidence,
    };

    Ok(Versioned::new(observation, version))
}

fn unpack_value(
    v: Option<&ObservationValue>,
) -> (
    Option<f64>,
    Option<String>,
    Option<String>,
    Option<&'static str>,
    Option<String>,
) {
    match v {
        None => (None, None, None, None, None),
        Some(ObservationValue::Quantity(q)) => {
            (Some(q.value), Some(q.unit.clone()), None, None, None)
        }
        Some(ObservationValue::String(s)) => (None, None, Some(s.clone()), None, None),
        Some(ObservationValue::Codeable { code_system, code }) => (
            None,
            None,
            None,
            Some(code_system.as_str()),
            Some(code.clone()),
        ),
    }
}
