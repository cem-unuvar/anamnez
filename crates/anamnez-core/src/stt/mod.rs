//! Speech-to-text trait. Production: local MLX inference. Tests: fixture-backed.

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttCall {
    pub provider_id: String,
    pub model_id: String,
    pub audio_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttResponse {
    pub text: String,
}

#[async_trait]
pub trait Transcriber: Send + Sync + 'static {
    async fn transcribe(&self, call: SttCall, audio: &[u8]) -> Result<SttResponse>;
}
