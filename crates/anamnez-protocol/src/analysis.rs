use crate::ids::{ObservationId, PatientAnalysisId, PatientId, UserId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientAnalysis {
    pub id: PatientAnalysisId,
    pub patient_id: PatientId,
    pub generated_at: Timestamp,
    pub generated_by: UserId,
    pub model_id: String,
    pub prompt_version: String,
    pub report_markdown: String,
    pub scope_observation_ids: Vec<ObservationId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateAnalysisRequest {
    pub provider_id: String,
    pub model_id: String,
}
