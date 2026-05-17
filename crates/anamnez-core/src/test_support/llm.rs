//! Fixture-backed `LlmExtractor`. Asserts `temperature == 0` at the seam.

use super::fixture_cache::FixtureCache;
use crate::error::Result;
use crate::llm::cache_key::CacheKey;
use crate::llm::{LlmCall, LlmExtractor, LlmResponse};
use async_trait::async_trait;

pub struct FixtureLlmExtractor {
    pub cache: FixtureCache,
}

impl FixtureLlmExtractor {
    #[must_use]
    pub fn new(cache: FixtureCache) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl LlmExtractor for FixtureLlmExtractor {
    async fn complete(&self, call: LlmCall) -> Result<LlmResponse> {
        assert!(
            call.temperature == 0.0,
            "FixtureLlmExtractor: temperature must be 0.0 under test (got {})",
            call.temperature
        );
        let combined_prompt = format!("{}\n---\n{}", call.system_prompt, call.user_prompt);
        let key = CacheKey::compose(
            &call.provider_id,
            &call.model_id,
            &combined_prompt,
            r#"{"temperature":0}"#,
        );
        let v = self.cache.get(&key.hex())?;
        let text = v
            .get("text")
            .and_then(|x| x.as_str())
            .ok_or(crate::error::Error::Invariant(
                "fixture LLM JSON missing string field `text`",
            ))?
            .to_owned();
        Ok(LlmResponse { text })
    }
}
