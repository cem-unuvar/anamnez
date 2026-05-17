//! OCR trait. Production: Apple Vision via FFI (Phase 6). Tests: fixture-backed.

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrCall {
    pub provider_id: String,
    pub model_id: String,
    pub image_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResponse {
    pub text: String,
    pub blocks: Vec<OcrBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBlock {
    pub text: String,
    pub bbox: (f32, f32, f32, f32),
    pub confidence: f32,
}

#[async_trait]
pub trait OcrEngine: Send + Sync + 'static {
    async fn ocr(&self, call: OcrCall, bytes: &[u8]) -> Result<OcrResponse>;
}
