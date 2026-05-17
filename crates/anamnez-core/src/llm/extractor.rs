//! Observation extraction from source documents. README §Storage → Autocomplete and LLM extraction:
//! "The extractor never invents codes — if no match exists, `code_system` and `code`
//! are `null`, the `display_text` is preserved, and a human reviewer assigns the code
//! before the observation moves from `preliminary` to `final`."

use super::{ExtractedObservation, LlmCall, LlmExtractor};
use crate::error::{Error, Result};
use std::sync::Arc;

pub struct Extractor {
    llm: Arc<dyn LlmExtractor>,
    provider_id: String,
    model_id: String,
}

impl Extractor {
    pub fn new(llm: Arc<dyn LlmExtractor>, provider_id: String, model_id: String) -> Self {
        Self {
            llm,
            provider_id,
            model_id,
        }
    }

    /// Build the prompt, call the LLM, and parse a JSON array of candidate observations.
    /// Temperature is pinned to 0 — asserted at the seam.
    pub async fn extract_observations(
        &self,
        source_text: &str,
        patient_context_json: &str,
    ) -> Result<Vec<ExtractedObservation>> {
        let system_prompt = "Extract candidate observations from the source text. \
            Return a JSON array. Never invent codes — if no lookup match exists, \
            set code and code_system to null and preserve display_text verbatim from the source.";
        let user_prompt = format!(
            "PATIENT_CONTEXT:\n{}\n\nSOURCE_TEXT:\n{}",
            patient_context_json, source_text
        );
        let response = self
            .llm
            .complete(LlmCall {
                provider_id: self.provider_id.clone(),
                model_id: self.model_id.clone(),
                system_prompt: system_prompt.to_owned(),
                user_prompt,
                temperature: 0.0,
            })
            .await?;
        let parsed: Vec<ExtractedObservation> = serde_json::from_str(&response.text)
            .map_err(|e| Error::Invariant(string_leak(&format!("extractor JSON parse: {e}"))))?;
        Ok(parsed)
    }
}

fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
