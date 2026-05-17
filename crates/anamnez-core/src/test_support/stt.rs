//! Fixture-backed `Transcriber`.

use super::fixture_cache::FixtureCache;
use crate::error::{Error, Result};
use crate::llm::cache_key::CacheKey;
use crate::stt::{SttCall, SttResponse, Transcriber};
use async_trait::async_trait;

pub struct FixtureTranscriber {
    pub cache: FixtureCache,
}

impl FixtureTranscriber {
    #[must_use]
    pub fn new(cache: FixtureCache) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl Transcriber for FixtureTranscriber {
    async fn transcribe(&self, call: SttCall, _audio: &[u8]) -> Result<SttResponse> {
        let key = CacheKey::compose(&call.provider_id, &call.model_id, &call.audio_sha256, "{}");
        let v = self.cache.get(&key.hex())?;
        let resp: SttResponse = serde_json::from_value(v)
            .map_err(|e| Error::Invariant(string_leak(&format!("fixture STT shape: {e}"))))?;
        Ok(resp)
    }
}

fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
