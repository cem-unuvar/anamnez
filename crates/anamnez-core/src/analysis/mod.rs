//! README §Analysis — per-patient single-call LLM analysis.

pub mod json_shape;
pub mod prompt;

use crate::audit::{self, Action, AppendInput};
use crate::db::Database;
use crate::error::Result;
use crate::ids::{ObservationId, PatientAnalysisId, PatientId, UserId};
use crate::llm::{LlmCall, LlmExtractor};
use crate::patient_access::level_for_in_conn;
use jiff::Timestamp;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

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

/// Build patient JSON, call the LLM with the fixed system prompt, persist
/// `patient_analysis` row, emit `analysis.generate` audit.
pub async fn generate(
    db: &Database,
    llm: Arc<dyn LlmExtractor>,
    patient_id: PatientId,
    actor: UserId,
    provider_id: &str,
    model_id: &str,
) -> Result<PatientAnalysis> {
    // Read-side: build JSON and verify access.
    let (patient_json, scope_ids) = db.with_reader(|conn| {
        let lvl = level_for_in_conn(conn, actor, patient_id)?;
        if lvl.is_none() {
            return Err(crate::error::Error::NotFound);
        }
        let json = json_shape::build(db, patient_id)?;
        let mut stmt = conn.prepare(
            "SELECT id FROM observation WHERE patient_id = ?1 ORDER BY effective_period_start ASC",
        )?;
        let ids = stmt
            .query_map(params![patient_id.as_uuid().to_string()], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let mut parsed = Vec::with_capacity(ids.len());
        for s in ids {
            let uuid = uuid::Uuid::parse_str(&s)
                .map_err(|_| crate::error::Error::Invariant("observation.id parse"))?;
            parsed.push(ObservationId(uuid));
        }
        Ok((json, parsed))
    })?;

    let response = llm
        .complete(LlmCall {
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
            system_prompt: prompt::SYSTEM_PROMPT.to_owned(),
            user_prompt: serde_json::to_string(&patient_json)?,
            temperature: 0.0,
        })
        .await?;

    let now = db.clock().now();
    let id = PatientAnalysisId::new();
    let analysis = PatientAnalysis {
        id,
        patient_id,
        generated_at: now,
        generated_by: actor,
        model_id: model_id.to_owned(),
        prompt_version: prompt::PROMPT_VERSION.to_owned(),
        report_markdown: response.text,
        scope_observation_ids: scope_ids.clone(),
    };

    db.with_writer(|conn| {
        let scope_json = serde_json::to_string(
            &scope_ids
                .iter()
                .map(|o| o.as_uuid().to_string())
                .collect::<Vec<_>>(),
        )?;
        conn.execute(
            "INSERT INTO patient_analysis \
             (id, patient_id, generated_at, generated_by, model_id, prompt_version, report_markdown, scope_observation_ids) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                analysis.id.as_uuid().to_string(),
                analysis.patient_id.as_uuid().to_string(),
                analysis.generated_at.to_string(),
                analysis.generated_by.as_uuid().to_string(),
                analysis.model_id,
                analysis.prompt_version,
                analysis.report_markdown,
                scope_json,
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(actor),
                auth_session_id: None,
                action: Action::AnalysisGenerate,
                target_type: "patient_analysis".into(),
                target_id: analysis.id.as_uuid().to_string(),
                patient_id: Some(patient_id),
                metadata: json!({
                    "model_id": analysis.model_id,
                    "prompt_version": analysis.prompt_version,
                    "scope_size": analysis.scope_observation_ids.len(),
                }),
            },
        )?;
        Ok(())
    })?;

    Ok(analysis)
}
