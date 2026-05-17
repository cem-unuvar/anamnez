//! /v1/patients/:id/analysis — placeholder; full LLM pipeline lands in a follow-up.
//! Requires an `LlmExtractor` impl that the daemon doesn't yet plumb through.

use crate::serve::app_state::AppState;
use crate::serve::error::ApiError;
use anamnez_core::Error;
use axum::routing::post;
use axum::Router;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/patients/:id/analysis", post(generate))
}

async fn generate() -> std::result::Result<(), ApiError> {
    // The LLM extractor isn't wired into `AppState` for this slice. Returning 422
    // makes the unimplemented surface explicit to clients without crashing.
    Err(ApiError(Error::Invariant(
        "analysis endpoint not yet wired (LLM extractor missing in this build)",
    )))
}
