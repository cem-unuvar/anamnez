//! /v1/encounters/...

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::encounter;
use anamnez_core::ids::{EncounterId, PatientId};
use anamnez_protocol::encounter as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/encounters", post(start))
        .route("/v1/encounters/:id/finish", post(finish))
        .route("/v1/encounters/:id/cancel", post(cancel))
        .route("/v1/patients/:id/encounters", get(list_by_patient))
}

async fn list_by_patient(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Vec<Versioned<p::Encounter>>>, ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    let rows = encounter::list_by_patient(&state.db, auth.user_id(), pid)?;
    Ok(Json(
        rows.into_iter()
            .map(|v| Versioned::new(v.value.into(), v.version))
            .collect(),
    ))
}

async fn start(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<p::StartEncounterRequest>,
) -> std::result::Result<Json<Versioned<p::Encounter>>, ApiError> {
    let v = encounter::start(
        &state.db,
        req.patient_id.into(),
        auth.user_id(),
        req.kind.into(),
        req.reason_text,
    )?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn finish(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::FinishEncounterRequest>,
) -> std::result::Result<Json<Versioned<p::Encounter>>, ApiError> {
    let id = EncounterId(parse_uuid(&id_str)?);
    let v = encounter::finish(
        &state.db,
        auth.user_id(),
        id,
        req.expected_version,
        req.reason_code,
        req.reason_code_system.into(),
    )?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn cancel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::CancelEncounterRequest>,
) -> std::result::Result<Json<Versioned<p::Encounter>>, ApiError> {
    let id = EncounterId(parse_uuid(&id_str)?);
    let v = encounter::cancel(&state.db, auth.user_id(), id, req.expected_version)?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}
