//! README §Data Modelling — `patient` table + KVKK-suppression mechanics.

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::env::Environment;
use crate::error::{Error, Result};
use crate::ids::{PatientId, UserId};
use crate::locking::Versioned;
use crate::patient_access::{self, caps, level_for_in_conn};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SexAssignedAtBirth {
    Female,
    Male,
    Intersex,
    Unknown,
}

impl SexAssignedAtBirth {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Female => "female",
            Self::Male => "male",
            Self::Intersex => "intersex",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "female" => Ok(Self::Female),
            "male" => Ok(Self::Male),
            "intersex" => Ok(Self::Intersex),
            "unknown" => Ok(Self::Unknown),
            _ => Err(Error::Invariant("unknown sex_assigned_at_birth value")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    pub id: PatientId,
    pub mrn: Option<String>,
    pub given_names: String,
    pub family_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: jiff::civil::Date,
    pub sex_assigned_at_birth: SexAssignedAtBirth,
    pub gender_identity: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub created_by: UserId,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub deceased_at: Option<Timestamp>,
    pub archived_at: Option<Timestamp>,
    pub suppressed_at: Option<Timestamp>,
    pub suppression_reason: Option<String>,
    pub notice_acknowledged_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPatient {
    pub mrn: Option<String>,
    pub given_names: String,
    pub family_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: jiff::civil::Date,
    pub sex_assigned_at_birth: SexAssignedAtBirth,
    pub gender_identity: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relationship: Option<String>,
}

/// Create a patient. The `creator` is auto-inserted into `patient_access` at level
/// `owner`. Under `Environment::Test`, `given_names` and `family_name` must begin
/// with `[TEST]` — else `Error::TestPrefixRequired`.
pub fn create(db: &Database, creator: UserId, input: NewPatient) -> Result<Versioned<Patient>> {
    if matches!(db.env(), Environment::Test) {
        if !input.given_names.starts_with("[TEST]") || !input.family_name.starts_with("[TEST]") {
            return Err(Error::TestPrefixRequired);
        }
    }
    let id = PatientId::new();
    let now = db.clock().now();
    let patient = Patient {
        id,
        mrn: input.mrn.clone(),
        given_names: input.given_names.clone(),
        family_name: input.family_name.clone(),
        preferred_name: input.preferred_name.clone(),
        date_of_birth: input.date_of_birth,
        sex_assigned_at_birth: input.sex_assigned_at_birth,
        gender_identity: input.gender_identity.clone(),
        email: input.email.clone(),
        phone: input.phone.clone(),
        address: input.address.clone(),
        emergency_contact_name: input.emergency_contact_name.clone(),
        emergency_contact_phone: input.emergency_contact_phone.clone(),
        emergency_contact_relationship: input.emergency_contact_relationship.clone(),
        created_by: creator,
        created_at: now,
        updated_at: now,
        deceased_at: None,
        archived_at: None,
        suppressed_at: None,
        suppression_reason: None,
        notice_acknowledged_at: None,
    };

    db.with_writer(|conn| {
        conn.execute(
            "INSERT INTO patient \
             (id, mrn, given_names, family_name, preferred_name, date_of_birth, sex_assigned_at_birth, \
              gender_identity, email, phone, address, emergency_contact_name, emergency_contact_phone, \
              emergency_contact_relationship, created_by, created_at, updated_at, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 1)",
            params![
                patient.id.as_uuid().to_string(),
                patient.mrn,
                patient.given_names,
                patient.family_name,
                patient.preferred_name,
                patient.date_of_birth.to_string(),
                patient.sex_assigned_at_birth.as_str(),
                patient.gender_identity,
                patient.email,
                patient.phone,
                patient.address,
                patient.emergency_contact_name,
                patient.emergency_contact_phone,
                patient.emergency_contact_relationship,
                patient.created_by.as_uuid().to_string(),
                patient.created_at.to_string(),
                patient.updated_at.to_string(),
            ],
        )?;
        patient_access::insert_creator_as_owner_in_conn(conn, patient.id, creator)?;
        Ok(())
    })?;

    Ok(Versioned::new(patient, 1))
}

/// Read a patient. Returns `Error::NotFound` if `viewer` has no `patient_access` row.
/// README §Tenancy: existence is hidden from users without access.
pub fn get(db: &Database, viewer: UserId, id: PatientId) -> Result<Versioned<Patient>> {
    db.with_reader(|conn| {
        let level = patient_access::level_for_in_conn(conn, viewer, id)?;
        if level.is_none() {
            return Err(Error::NotFound);
        }
        let row = conn
            .query_row(
                "SELECT id, mrn, given_names, family_name, preferred_name, date_of_birth, \
                        sex_assigned_at_birth, gender_identity, email, phone, address, \
                        emergency_contact_name, emergency_contact_phone, emergency_contact_relationship, \
                        created_by, created_at, updated_at, deceased_at, archived_at, suppressed_at, \
                        suppression_reason, notice_acknowledged_at, version \
                 FROM patient WHERE id = ?1 AND suppressed_at IS NULL",
                params![id.as_uuid().to_string()],
                row_to_patient,
            )
            .optional()?;
        row.ok_or(Error::NotFound)
    })
}

fn row_to_patient(row: &rusqlite::Row<'_>) -> rusqlite::Result<Versioned<Patient>> {
    let id_str: String = row.get(0)?;
    let dob_str: String = row.get(5)?;
    let sex_str: String = row.get(6)?;
    let created_by_str: String = row.get(14)?;
    let created_at_str: String = row.get(15)?;
    let updated_at_str: String = row.get(16)?;

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

    let optional_ts = |idx: usize| -> rusqlite::Result<Option<Timestamp>> {
        let v: Option<String> = row.get(idx)?;
        match v {
            None => Ok(None),
            Some(s) => Ok(Some(parse_ts(&s)?)),
        }
    };

    let patient = Patient {
        id: PatientId(parse_uuid(&id_str)?),
        mrn: row.get(1)?,
        given_names: row.get(2)?,
        family_name: row.get(3)?,
        preferred_name: row.get(4)?,
        date_of_birth: jiff::civil::Date::strptime("%Y-%m-%d", &dob_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        sex_assigned_at_birth: SexAssignedAtBirth::parse(&sex_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        gender_identity: row.get(7)?,
        email: row.get(8)?,
        phone: row.get(9)?,
        address: row.get(10)?,
        emergency_contact_name: row.get(11)?,
        emergency_contact_phone: row.get(12)?,
        emergency_contact_relationship: row.get(13)?,
        created_by: UserId(parse_uuid(&created_by_str)?),
        created_at: parse_ts(&created_at_str)?,
        updated_at: parse_ts(&updated_at_str)?,
        deceased_at: optional_ts(17)?,
        archived_at: optional_ts(18)?,
        suppressed_at: optional_ts(19)?,
        suppression_reason: row.get(20)?,
        notice_acknowledged_at: optional_ts(21)?,
    };
    let version: i64 = row.get(22)?;
    Ok(Versioned::new(patient, version))
}

/// In-place patient update: applies `PatientPatch`, bumps `updated_at` and `version`.
/// Optimistic locking on `expected_version`; stale → `Error::Conflict`.
pub fn update(
    db: &Database,
    actor: UserId,
    id: PatientId,
    expected_version: i64,
    patch: PatientPatch,
) -> Result<Versioned<Patient>> {
    db.with_writer(|conn| {
        let current = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;

        let lvl = level_for_in_conn(conn, actor, id)?;
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
        if let Some(v) = patch.mrn {
            next.mrn = v;
        }
        if let Some(v) = patch.preferred_name {
            next.preferred_name = v;
        }
        if let Some(v) = patch.email {
            next.email = v;
        }
        if let Some(v) = patch.phone {
            next.phone = v;
        }
        if let Some(v) = patch.address {
            next.address = v;
        }
        if let Some(v) = patch.deceased_at {
            next.deceased_at = v;
        }
        if let Some(v) = patch.archived_at {
            next.archived_at = v;
        }
        if let Some(v) = patch.notice_acknowledged_at {
            next.notice_acknowledged_at = v;
        }
        next.updated_at = db.clock().now();

        let affected = conn.execute(
            "UPDATE patient SET \
             mrn = ?2, preferred_name = ?3, email = ?4, phone = ?5, address = ?6, \
             deceased_at = ?7, archived_at = ?8, notice_acknowledged_at = ?9, updated_at = ?10, \
             version = version + 1 \
             WHERE id = ?1 AND version = ?11",
            params![
                next.id.as_uuid().to_string(),
                next.mrn,
                next.preferred_name,
                next.email,
                next.phone,
                next.address,
                next.deceased_at.map(|t| t.to_string()),
                next.archived_at.map(|t| t.to_string()),
                next.notice_acknowledged_at.map(|t| t.to_string()),
                next.updated_at.to_string(),
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
                action: Action::PatientUpdate,
                target_type: "patient".into(),
                target_id: id.as_uuid().to_string(),
                patient_id: Some(id),
                metadata: json!({"new_version": expected_version + 1}),
            },
        )?;

        Ok(Versioned::new(next, expected_version + 1))
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatientPatch {
    pub mrn: Option<Option<String>>,
    pub preferred_name: Option<Option<String>>,
    pub email: Option<Option<String>>,
    pub phone: Option<Option<String>>,
    pub address: Option<Option<String>>,
    pub deceased_at: Option<Option<Timestamp>>,
    pub archived_at: Option<Option<Timestamp>>,
    pub notice_acknowledged_at: Option<Option<Timestamp>>,
}

fn load_in_conn(conn: &rusqlite::Connection, id: PatientId) -> Result<Option<Versioned<Patient>>> {
    let row = conn
        .query_row(
            "SELECT id, mrn, given_names, family_name, preferred_name, date_of_birth, \
                    sex_assigned_at_birth, gender_identity, email, phone, address, \
                    emergency_contact_name, emergency_contact_phone, emergency_contact_relationship, \
                    created_by, created_at, updated_at, deceased_at, archived_at, suppressed_at, \
                    suppression_reason, notice_acknowledged_at, version \
             FROM patient WHERE id = ?1 AND suppressed_at IS NULL",
            params![id.as_uuid().to_string()],
            row_to_patient,
        )
        .optional()?;
    Ok(row)
}

/// Lightweight projection for the list view: demographics + the caller's
/// access level. Suppressed rows are filtered out at the SQL level.
#[derive(Debug, Clone)]
pub struct PatientListRow {
    pub id: PatientId,
    pub mrn: Option<String>,
    pub given_names: String,
    pub family_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: jiff::civil::Date,
    pub sex_assigned_at_birth: SexAssignedAtBirth,
    pub access_level: crate::patient_access::AccessLevel,
    pub updated_at: Timestamp,
    pub deceased_at: Option<Timestamp>,
    pub archived_at: Option<Timestamp>,
}

#[derive(Debug, Clone, Default)]
pub struct PatientListQuery {
    /// Free-text filter applied case-insensitively against given_names, family_name,
    /// and mrn. NFC-normalized in the caller; passed through here verbatim.
    pub q: Option<String>,
    /// If false, `archived_at IS NOT NULL` rows are excluded.
    pub include_archived: bool,
    /// Caller-requested limit; clamped to [1, 200].
    pub limit: Option<u32>,
}

const PATIENT_LIST_MAX: u32 = 200;

/// List patients the viewer has any `patient_access` row for. Suppressed rows
/// are always excluded (KVKK m. 11/e — invisible everywhere except audit and
/// the retention sweep). Results are ordered by most-recently-updated first.
pub fn list_for_user(
    db: &Database,
    viewer: UserId,
    query: PatientListQuery,
) -> Result<Vec<PatientListRow>> {
    db.with_reader(|conn| {
        let limit = query.limit.unwrap_or(PATIENT_LIST_MAX).min(PATIENT_LIST_MAX);
        let q_norm = query.q.as_ref().map(|s| s.to_lowercase());
        let like_pat = q_norm.as_ref().map(|s| format!("%{s}%"));
        let mut sql = String::from(
            "SELECT p.id, p.mrn, p.given_names, p.family_name, p.preferred_name, p.date_of_birth, \
                    p.sex_assigned_at_birth, pa.level, p.updated_at, p.deceased_at, p.archived_at \
             FROM patient p \
             INNER JOIN patient_access pa ON pa.patient_id = p.id \
             WHERE pa.user_id = ?1 AND p.suppressed_at IS NULL",
        );
        if !query.include_archived {
            sql.push_str(" AND p.archived_at IS NULL");
        }
        if like_pat.is_some() {
            sql.push_str(
                " AND (lower(p.given_names) LIKE ?2 OR lower(p.family_name) LIKE ?2 \
                       OR (p.mrn IS NOT NULL AND lower(p.mrn) LIKE ?2))",
            );
        }
        sql.push_str(" ORDER BY p.updated_at DESC LIMIT ?");
        sql.push_str(if like_pat.is_some() { "3" } else { "2" });

        let mut stmt = conn.prepare(&sql)?;
        let mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<PatientListRow> {
            let id_str: String = row.get(0)?;
            let dob_str: String = row.get(5)?;
            let sex_str: String = row.get(6)?;
            let level_str: String = row.get(7)?;
            let updated_at_str: String = row.get(8)?;

            let parse_uuid = |s: &str| {
                uuid::Uuid::parse_str(s).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            };
            let parse_ts = |s: &str| -> rusqlite::Result<Timestamp> {
                s.parse().map_err(|e: jiff::Error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            };
            let optional_ts = |idx: usize| -> rusqlite::Result<Option<Timestamp>> {
                let v: Option<String> = row.get(idx)?;
                match v {
                    None => Ok(None),
                    Some(s) => Ok(Some(parse_ts(&s)?)),
                }
            };

            Ok(PatientListRow {
                id: PatientId(parse_uuid(&id_str)?),
                mrn: row.get(1)?,
                given_names: row.get(2)?,
                family_name: row.get(3)?,
                preferred_name: row.get(4)?,
                date_of_birth: jiff::civil::Date::strptime("%Y-%m-%d", &dob_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                sex_assigned_at_birth: SexAssignedAtBirth::parse(&sex_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                access_level: crate::patient_access::AccessLevel::parse(&level_str).map_err(
                    |e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    },
                )?,
                updated_at: parse_ts(&updated_at_str)?,
                deceased_at: optional_ts(9)?,
                archived_at: optional_ts(10)?,
            })
        };
        let rows: Vec<PatientListRow> = match (&like_pat, limit) {
            (Some(p), l) => stmt
                .query_map(params![viewer.as_uuid().to_string(), p, l], mapper)?
                .collect::<rusqlite::Result<Vec<_>>>()?,
            (None, l) => stmt
                .query_map(params![viewer.as_uuid().to_string(), l], mapper)?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        };
        Ok(rows)
    })
}
