//! Fixture-backed `OcrEngine`.

use super::fixture_cache::FixtureCache;
use crate::error::{Error, Result};
use crate::llm::cache_key::CacheKey;
use crate::ocr::{OcrCall, OcrEngine, OcrResponse};
use async_trait::async_trait;

pub struct FixtureOcrEngine {
    pub cache: FixtureCache,
}

impl FixtureOcrEngine {
    #[must_use]
    pub fn new(cache: FixtureCache) -> Self {
        Self { cache }
    }
}

#[async_trait]
impl OcrEngine for FixtureOcrEngine {
    async fn ocr(&self, call: OcrCall, _bytes: &[u8]) -> Result<OcrResponse> {
        let key = CacheKey::compose(&call.provider_id, &call.model_id, &call.image_sha256, "{}");
        let v = self.cache.get(&key.hex())?;
        let resp: OcrResponse = serde_json::from_value(v)
            .map_err(|e| Error::Invariant(string_leak(&format!("fixture OCR shape: {e}"))))?;
        Ok(resp)
    }
}

fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
