//! /v1/patients[/...]

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use anamnez_core::ids::PatientId;
use anamnez_core::patient;
use anamnez_protocol::patient as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/patients", post(create))
        .route("/v1/patients/:id", get(get_one).patch(update))
}

async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(input): Json<p::NewPatient>,
) -> std::result::Result<Json<Versioned<p::Patient>>, ApiError> {
    let v = patient::create(&state.db, auth.user_id(), input.into())?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn get_one(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Versioned<p::Patient>>, ApiError> {
    let id = PatientId(parse_uuid(&id_str)?);
    let v = patient::get(&state.db, auth.user_id(), id)?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::UpdatePatientRequest>,
) -> std::result::Result<Json<Versioned<p::Patient>>, ApiError> {
    let id = PatientId(parse_uuid(&id_str)?);
    let v = patient::update(
        &state.db,
        auth.user_id(),
        id,
        req.expected_version,
        req.patch.into(),
    )?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

pub(crate) fn parse_uuid(s: &str) -> std::result::Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(s).map_err(|_| ApiError(anamnez_core::Error::NotFound))
}
