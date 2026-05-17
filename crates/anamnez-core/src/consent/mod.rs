//! README §Compliance → KVKK-derived features — `patient_consent` table for the
//! narrow set of flows that require explicit consent beyond KVKK m. 6/3.

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{PatientConsentId, PatientId, SourceDocumentId, UserId};
use crate::locking::Versioned;
use crate::patient_access::{caps, level_for_in_conn};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentPurpose {
    LawyerTransfer,
    ResearchNonAnonymized,
    OtherClinicReferral,
}

impl ConsentPurpose {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LawyerTransfer => "lawyer_transfer",
            Self::ResearchNonAnonymized => "research_non_anonymized",
            Self::OtherClinicReferral => "other_clinic_referral",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "lawyer_transfer" => Ok(Self::LawyerTransfer),
            "research_non_anonymized" => Ok(Self::ResearchNonAnonymized),
            "other_clinic_referral" => Ok(Self::OtherClinicReferral),
            _ => Err(Error::Invariant("unknown consent purpose")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientConsent {
    pub id: PatientConsentId,
    pub patient_id: PatientId,
    pub purpose: ConsentPurpose,
    pub granted_at: Timestamp,
    pub granted_by: UserId,
    pub evidence_source_id: Option<SourceDocumentId>,
    pub revoked_at: Option<Timestamp>,
    pub notes: Option<String>,
}

/// Record an explicit consent. Requires collaborator-or-better on the patient.
pub fn record(
    db: &Database,
    actor: UserId,
    patient_id: PatientId,
    purpose: ConsentPurpose,
    evidence_source_id: Option<SourceDocumentId>,
    notes: Option<String>,
) -> Result<Versioned<PatientConsent>> {
    let id = PatientConsentId::new();
    let now = db.clock().now();

    let consent = PatientConsent {
        id,
        patient_id,
        purpose,
        granted_at: now,
        granted_by: actor,
        evidence_source_id,
        revoked_at: None,
        notes: notes.clone(),
    };

    db.with_writer(|conn| {
        let lvl = level_for_in_conn(conn, actor, patient_id)?;
        match lvl {
            Some(l) if caps::can_write_clinical(l) => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }

        conn.execute(
            "INSERT INTO patient_consent \
             (id, patient_id, purpose, granted_at, granted_by, evidence_source_id, notes, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
            params![
                consent.id.as_uuid().to_string(),
                consent.patient_id.as_uuid().to_string(),
                consent.purpose.as_str(),
                consent.granted_at.to_string(),
                consent.granted_by.as_uuid().to_string(),
                consent.evidence_source_id.map(|i| i.as_uuid().to_string()),
                consent.notes,
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::ConsentRecord,
                target_type: "patient_consent".into(),
                target_id: consent.id.as_uuid().to_string(),
                patient_id: Some(consent.patient_id),
                metadata: json!({"purpose": consent.purpose.as_str()}),
            },
        )?;
        Ok(())
    })?;

    Ok(Versioned::new(consent, 1))
}

/// Revoke an existing consent by setting `revoked_at`. Optimistic locking.
pub fn revoke(
    db: &Database,
    actor: UserId,
    id: PatientConsentId,
    expected_version: i64,
) -> Result<Versioned<PatientConsent>> {
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
        let now = db.clock().now();
        next.revoked_at = Some(now);

        let affected = conn.execute(
            "UPDATE patient_consent SET revoked_at = ?2, version = version + 1 \
             WHERE id = ?1 AND version = ?3",
            params![
                next.id.as_uuid().to_string(),
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
                action: Action::ConsentRevoke,
                target_type: "patient_consent".into(),
                target_id: next.id.as_uuid().to_string(),
                patient_id: Some(next.patient_id),
                metadata: json!({"new_version": expected_version + 1}),
            },
        )?;

        Ok(Versioned::new(next, expected_version + 1))
    })
}

/// Returns `true` if there is a non-revoked consent of `purpose` for `patient_id`.
pub fn has_active(db: &Database, patient_id: PatientId, purpose: ConsentPurpose) -> Result<bool> {
    db.with_reader(|conn| {
        let row: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM patient_consent \
                 WHERE patient_id = ?1 AND purpose = ?2 AND revoked_at IS NULL LIMIT 1",
                params![patient_id.as_uuid().to_string(), purpose.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(row.is_some())
    })
}

fn load_in_conn(
    conn: &rusqlite::Connection,
    id: PatientConsentId,
) -> Result<Option<Versioned<PatientConsent>>> {
    let row = conn
        .query_row(
            "SELECT id, patient_id, purpose, granted_at, granted_by, evidence_source_id, \
                    revoked_at, notes, version \
             FROM patient_consent WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_consent,
        )
        .optional()?;
    Ok(row)
}

fn row_to_consent(row: &rusqlite::Row<'_>) -> rusqlite::Result<Versioned<PatientConsent>> {
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
    let purpose_str: String = row.get(2)?;
    let granted_at_str: String = row.get(3)?;
    let granted_by_str: String = row.get(4)?;
    let evidence_source_id_str: Option<String> = row.get(5)?;
    let revoked_at_str: Option<String> = row.get(6)?;
    let notes: Option<String> = row.get(7)?;
    let version: i64 = row.get(8)?;

    let consent = PatientConsent {
        id: PatientConsentId(parse_uuid(&id_str)?),
        patient_id: PatientId(parse_uuid(&patient_id_str)?),
        purpose: ConsentPurpose::parse(&purpose_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        granted_at: parse_ts(&granted_at_str)?,
        granted_by: UserId(parse_uuid(&granted_by_str)?),
        evidence_source_id: match evidence_source_id_str {
            None => None,
            Some(s) => Some(SourceDocumentId(parse_uuid(&s)?)),
        },
        revoked_at: match revoked_at_str {
            None => None,
            Some(s) => Some(parse_ts(&s)?),
        },
        notes,
    };

    Ok(Versioned::new(consent, version))
}
