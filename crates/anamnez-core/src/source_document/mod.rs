//! README §Data Modelling / §Storage → Source documents — `source_document` row.
//!
//! The bytes live in the `BlobStore` keyed by `sha256`; the SQLite row carries
//! sha256, filename, MIME type. `create` only writes the DB row — the caller is
//! expected to have already written bytes through `BlobStore` (separate module,
//! separate test surface).

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::{EncounterId, PatientId, SourceDocumentId, UserId};
use crate::locking::Versioned;
use crate::patient_access::{caps, level_for_in_conn};
use jiff::Timestamp;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDocumentType {
    Note,
    Pdf,
    Audio,
    Image,
}

impl SourceDocumentType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Pdf => "pdf",
            Self::Audio => "audio",
            Self::Image => "image",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "note" => Ok(Self::Note),
            "pdf" => Ok(Self::Pdf),
            "audio" => Ok(Self::Audio),
            "image" => Ok(Self::Image),
            _ => Err(Error::Invariant("unknown source document type")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocument {
    pub id: SourceDocumentId,
    pub patient_id: PatientId,
    pub kind: SourceDocumentType,
    pub sha256: String,
    pub original_filename: String,
    pub mime_type: String,
    pub transcription: Option<String>,
    pub ocr_text: Option<String>,
    pub encounter_id: Option<EncounterId>,
    pub uploaded_at: Timestamp,
    pub context_provided_by_user: Option<String>,
    pub recorded_by: UserId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSourceDocument {
    pub patient_id: PatientId,
    pub kind: SourceDocumentType,
    pub sha256: String,
    pub original_filename: String,
    pub mime_type: String,
    pub transcription: Option<String>,
    pub ocr_text: Option<String>,
    pub encounter_id: Option<EncounterId>,
    pub context_provided_by_user: Option<String>,
}

/// Insert a `source_document` row. Caller is expected to have written the bytes
/// to `BlobStore` first; `sha256` ties the two. Requires collaborator-or-better.
pub fn create(
    db: &Database,
    actor: UserId,
    input: NewSourceDocument,
) -> Result<Versioned<SourceDocument>> {
    let id = SourceDocumentId::new();
    let now = db.clock().now();

    let doc = SourceDocument {
        id,
        patient_id: input.patient_id,
        kind: input.kind,
        sha256: input.sha256.clone(),
        original_filename: input.original_filename.clone(),
        mime_type: input.mime_type.clone(),
        transcription: input.transcription.clone(),
        ocr_text: input.ocr_text.clone(),
        encounter_id: input.encounter_id,
        uploaded_at: now,
        context_provided_by_user: input.context_provided_by_user.clone(),
        recorded_by: actor,
    };

    db.with_writer(|conn| {
        let lvl = level_for_in_conn(conn, actor, input.patient_id)?;
        match lvl {
            Some(l) if caps::can_write_clinical(l) => {}
            Some(_) => return Err(Error::Forbidden),
            None => return Err(Error::NotFound),
        }

        conn.execute(
            "INSERT INTO source_document \
             (id, patient_id, kind, sha256, original_filename, mime_type, transcription, ocr_text, \
              encounter_id, uploaded_at, context_provided_by_user, recorded_by, version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1)",
            params![
                doc.id.as_uuid().to_string(),
                doc.patient_id.as_uuid().to_string(),
                doc.kind.as_str(),
                doc.sha256,
                doc.original_filename,
                doc.mime_type,
                doc.transcription,
                doc.ocr_text,
                doc.encounter_id.map(|i| i.as_uuid().to_string()),
                doc.uploaded_at.to_string(),
                doc.context_provided_by_user,
                doc.recorded_by.as_uuid().to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::SourceDocumentCreate,
                target_type: "source_document".into(),
                target_id: doc.id.as_uuid().to_string(),
                patient_id: Some(doc.patient_id),
                metadata: json!({"kind": doc.kind.as_str(), "sha256": doc.sha256}),
            },
        )?;
        Ok(())
    })?;

    Ok(Versioned::new(doc, 1))
}

/// Read a source-document row. Requires any `patient_access` level.
pub fn get(
    db: &Database,
    viewer: UserId,
    id: SourceDocumentId,
) -> Result<Versioned<SourceDocument>> {
    db.with_reader(|conn| {
        let doc = load_in_conn(conn, id)?.ok_or(Error::NotFound)?;
        let lvl = level_for_in_conn(conn, viewer, doc.value.patient_id)?;
        match lvl {
            Some(l) if caps::can_read(l) => Ok(doc),
            _ => Err(Error::NotFound),
        }
    })
}

fn load_in_conn(
    conn: &rusqlite::Connection,
    id: SourceDocumentId,
) -> Result<Option<Versioned<SourceDocument>>> {
    let row = conn
        .query_row(
            "SELECT id, patient_id, kind, sha256, original_filename, mime_type, transcription, ocr_text, \
                    encounter_id, uploaded_at, context_provided_by_user, recorded_by, version \
             FROM source_document WHERE id = ?1",
            params![id.as_uuid().to_string()],
            row_to_source_document,
        )
        .optional()?;
    Ok(row)
}

fn row_to_source_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<Versioned<SourceDocument>> {
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
    let kind_str: String = row.get(2)?;
    let sha256: String = row.get(3)?;
    let original_filename: String = row.get(4)?;
    let mime_type: String = row.get(5)?;
    let transcription: Option<String> = row.get(6)?;
    let ocr_text: Option<String> = row.get(7)?;
    let encounter_id_str: Option<String> = row.get(8)?;
    let uploaded_at_str: String = row.get(9)?;
    let context_provided_by_user: Option<String> = row.get(10)?;
    let recorded_by_str: String = row.get(11)?;
    let version: i64 = row.get(12)?;

    let doc = SourceDocument {
        id: SourceDocumentId(parse_uuid(&id_str)?),
        patient_id: PatientId(parse_uuid(&patient_id_str)?),
        kind: SourceDocumentType::parse(&kind_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        sha256,
        original_filename,
        mime_type,
        transcription,
        ocr_text,
        encounter_id: match encounter_id_str {
            None => None,
            Some(s) => Some(EncounterId(parse_uuid(&s)?)),
        },
        uploaded_at: parse_ts(&uploaded_at_str)?,
        context_provided_by_user,
        recorded_by: UserId(parse_uuid(&recorded_by_str)?),
    };

    Ok(Versioned::new(doc, version))
}
