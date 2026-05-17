//! LLM extractor + patient analysis trait. Production impls land in Phase 6;
//! tests use fixture-backed impls under `test_support`.

pub mod cache_key;
pub mod extractor;

pub use cache_key::CacheKey;

use crate::code_systems::CodeSystem;
use crate::error::Result;
use async_trait::async_trait;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCall {
    pub provider_id: String,
    pub model_id: String,
    pub system_prompt: String,
    pub user_prompt: String,
    /// Pinned to `0.0` under `Environment::Test`. Asserted at the seam.
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub text: String,
}

#[async_trait]
pub trait LlmExtractor: Send + Sync + 'static {
    async fn complete(&self, call: LlmCall) -> Result<LlmResponse>;
}

/// LLM-proposed observation from a source document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedObservation {
    pub code_system: Option<CodeSystem>,
    pub code: Option<String>,
    pub display_text: String,
    pub text_span: (usize, usize),
    pub effective_period_start: Timestamp,
    pub effective_period_end: Option<Timestamp>,
    pub value_quantity: Option<(f64, String)>,
    pub value_string: Option<String>,
    pub value_codeable: Option<(CodeSystem, String)>,
    pub confidence: f64,
}
