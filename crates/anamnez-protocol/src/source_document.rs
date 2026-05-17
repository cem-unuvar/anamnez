use crate::ids::{EncounterId, PatientId, SourceDocumentId, UserId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDocumentType {
    Note,
    Pdf,
    Audio,
    Image,
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
