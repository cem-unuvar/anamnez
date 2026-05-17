//! README §Data Modelling — `allergy` table. `(code, code_system)` are co-nullable;
//! when set, `code_system` must be `ATC` (drug allergies only at MVP).

use crate::audit::{self, Action, AppendInput};
use crate::code_systems::{self, CodeSystem};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{AllergyId, EncounterId, PatientId, SourceDocumentId, UserId};
use crate::locking::Versioned;
use crate::patient_access::{caps, level_for_in_conn};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllergySeverity {
    Mild,
    Moderate,
    Severe,
    LifeThreatening,
}

impl AllergySeverity {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mild => "mild",
            Self::Moderate => "moderate",
            Self::Severe => "severe",
            Self::LifeThreatening => "life_threatening",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "mild" => Ok(Self::Mild),
            "moderate" => Ok(Self::Moderate),
            "severe" => Ok(Self::Severe),
            "life_threatening" => Ok(Self::LifeThreatening),
            _ => Err(Error::Invariant("unknown allergy severity")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllergyStatus {
    Active,
    Inactive,
    EnteredInError,
}

impl AllergyStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::EnteredInError => "entered_in_error",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "entered_in_error" => Ok(Self::EnteredInError),
            _ => Err(Error::Invariant("unknown allergy status")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allergy {
    pub id: AllergyId,
    pub patient_id: PatientId,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub severity: AllergySeverity,
    pub reaction_text: Option<String>,
    pub status: AllergyStatus,
    pub onset_date: Option<jiff::civil::Date>,
    pub recorded_at: Timestamp,
    pub recorded_by: UserId,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAllergy {
    pub patient_id: PatientId,
    pub code: Option<String>,
    pub code_system: Option<CodeSystem>,
    pub display_text: String,
    pub severity: AllergySeverity,
    pub reaction_text: Option<String>,
    pub status: AllergyStatus,
    pub onset_date: Option<jiff::civil::Date>,
    pub source_id: Option<SourceDocumentId>,
    pub encounter_id: Option<EncounterId>,
}

/// Validation:
/// - `(code, code_system)` are co-nullable.
/// - When set, `code_system` must be `Atc` (drug allergies at MVP).
/// - `(Atc, code)` pair must exist in `drug_atc`.
/// - Caller must have collaborator-or-better access to the patient.
pub fn create(db: &Database, actor: UserId, input: NewAllergy) -> Result<Versioned<Allergy>> {
    if input.code.is_some() != input.code_system.is_some() {
        return Err(Error::Invariant(
            "allergy (code, code_system) must be both set or both null",
        ));
    }
    if let Some(cs) = input.code_system {
        if !cs.is_allergy_scope() {
            return Err(Error::CodeSystemNotAllowed {
                code_system: cs.as_str().to_owned(),
                context: "allergy",
            });
        }
    }

    let id = AllergyId::new();
    let now = db.clock().now();

    let allergy = Allergy {
        id,
        patient_id: input.patient_id,
        code: input.code.clone(),
        code_system: input.code_system,
        display_text: input.display_text.clone(),
        severity: input.severity,
        reaction_text: input.reaction_text.clone(),
        status: input.status,
        onset_date: input.onset_date,
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

        if let (Some(cs), Some(code)) = (allergy.code_system, allergy.code.as_deref()) {
            code_systems::lookup_in_conn(conn, cs, code)?;
        }

        conn.execute(
            "INSERT INTO allergy \
             (id, patient_id, code, code_system, display_text, severity, reaction_text, status, \
              onset_date, recorded_at, recorded_by, source_id, encounter_id, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1)",
            params![
                allergy.id.as_uuid().to_string(),
                allergy.patient_id.as_uuid().to_string(),
                allergy.code,
                allergy.code_system.map(|c| c.as_str()),
                allergy.display_text,
                allergy.severity.as_str(),
                allergy.reaction_text,
                allergy.status.as_str(),
                allergy.onset_date.map(|d| d.to_string()),
                allergy.recorded_at.to_string(),
                allergy.recorded_by.as_uuid().to_string(),
                allergy.source_id.map(|i| i.as_uuid().to_string()),
                allergy.encounter_id.map(|i| i.as_uuid().to_string()),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::AllergyCreate,
                target_type: "allergy".into(),
                target_id: allergy.id.as_uuid().to_string(),
                patient_id: Some(allergy.patient_id),
                metadata: json!({"severity": allergy.severity.as_str(), "status": allergy.status.as_str()}),
            },
        )?;
        Ok(())
    })?;

    Ok(Versioned::new(allergy, 1))
}

/// In-place amendment with optimistic locking on `expected_version`.
pub fn amend(
    db: &Database,
    actor: UserId,
    id: AllergyId,
    expected_version: i64,
    patch: AllergyPatch,
) -> Result<Versioned<Allergy>> {
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
        if let Some(v) = patch.severity {
            next.severity = v;
        }
        if let Some(v) = patch.reaction_text {
            next.reaction_text = v;
        }
        if let Some(v) = patch.status {
            next.status = v;
        }
        if let Some(v) = patch.onset_date {
            next.onset_date = v;
        }
        next.recorded_at = db.clock().now();

        let affected = conn.execute(
            "UPDATE allergy SET \
             severity = ?2, reaction_text = ?3, status = ?4, onset_date = ?5, recorded_at = ?6, \
             version = version + 1 \
             WHERE id = ?1 AND version = ?7",
            params![
                next.id.as_uuid().to_string(),
                next.severity.as_str(),
                next.reaction_text,
                next.status.as_str(),
                next.onset_date.map(|d| d.to_string()),
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
                action: Action::AllergyAmend,
                target_type: "allergy".into(),
                target_id: next.id.as_uuid().to_string(),
                patient_id: Some(next.patient_id),
                metadata: json!({"new_version": expected_version + 1}),
            },
        )?;

        Ok(Versioned::new(next, expected_version + 1))
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AllergyPatch {
    pub severity: Option<AllergySeverity>,
    pub reaction_text: Option<Option<String>>,
    pub status: Option<AllergyStatus>,
    pub onset_date: Option<Option<jiff::civil::Date>>,
}

fn load_in_conn(conn: &rusqlite::Connection, id: AllergyId) -> Result<Option<Versioned<Allergy>>> {
    let row = conn
        .query_row(
            "SELECT id, patient_id, code, code_system, display_text, severity, reaction_text, status, \
                    onset_date, recorded_at, recorded_by, source_id, encounter_id, version \
             FROM allergy WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_allergy,
        )
        .optional()?;
    Ok(row)
}

fn row_to_allergy(row: &rusqlite::Row<'_>) -> rusqlite::Result<Versioned<Allergy>> {
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
    let code: Option<String> = row.get(2)?;
    let code_system_str: Option<String> = row.get(3)?;
    let display_text: String = row.get(4)?;
    let severity_str: String = row.get(5)?;
    let reaction_text: Option<String> = row.get(6)?;
    let status_str: String = row.get(7)?;
    let onset_date_str: Option<String> = row.get(8)?;
    let recorded_at_str: String = row.get(9)?;
    let recorded_by_str: String = row.get(10)?;
    let source_id_str: Option<String> = row.get(11)?;
    let encounter_id_str: Option<String> = row.get(12)?;
    let version: i64 = row.get(13)?;

    let code_system = match code_system_str {
        None => None,
        Some(s) => Some(CodeSystem::parse_tag(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?),
    };

    let allergy = Allergy {
        id: AllergyId(parse_uuid(&id_str)?),
        patient_id: PatientId(parse_uuid(&patient_id_str)?),
        code,
        code_system,
        display_text,
        severity: AllergySeverity::parse(&severity_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        reaction_text,
        status: AllergyStatus::parse(&status_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        onset_date: match onset_date_str {
            None => None,
            Some(s) => Some(jiff::civil::Date::strptime("%Y-%m-%d", &s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?),
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

    Ok(Versioned::new(allergy, version))
}
