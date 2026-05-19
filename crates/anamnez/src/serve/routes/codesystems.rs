//! /v1/codesystems/search — FTS5-backed autocomplete over the reference tables.
//! Folding to Turkish casefold happens inside `code_systems::autocomplete`.

use crate::serve::app_state::AppState;
use crate::serve::error::ApiError;
use anamnez_core::code_systems;
use anamnez_protocol::codesystem as p;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/codesystems/search", get(search))
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    system: p::CodeSystem,
    q: String,
    limit: Option<usize>,
}

async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> std::result::Result<Json<p::SearchResponse>, ApiError> {
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let rows = code_systems::autocomplete(&state.db, params.system.into(), &params.q, limit)?;
    let hits = rows.into_iter().map(Into::into).collect();
    Ok(Json(p::SearchResponse { hits }))
}
