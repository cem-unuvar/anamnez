//! /v1/observations + /v1/patients/:id/observations + problem-list

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::ids::{ObservationId, PatientId};
use anamnez_core::observation;
use anamnez_protocol::events::ServerEvent;
use anamnez_protocol::observation as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/observations", post(create))
        .route("/v1/observations/:id", get(get_one).patch(amend))
        .route("/v1/patients/:id/observations", get(list_by_patient))
        .route("/v1/patients/:id/problem-list", get(problem_list))
}

async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(input): Json<p::NewObservation>,
) -> std::result::Result<Json<Versioned<p::Observation>>, ApiError> {
    let v = observation::create(&state.db, auth.user_id(), input.into())?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn get_one(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Versioned<p::Observation>>, ApiError> {
    let id = ObservationId(parse_uuid(&id_str)?);
    let v = observation::get(&state.db, auth.user_id(), id)?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn amend(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::AmendObservationRequest>,
) -> std::result::Result<Json<Versioned<p::Observation>>, ApiError> {
    let id = ObservationId(parse_uuid(&id_str)?);
    let v = observation::amend(
        &state.db,
        auth.user_id(),
        id,
        req.expected_version,
        req.patch.into(),
    )?;
    // Emit SSE event so other connected workstations see the amend.
    let ev = ServerEvent::observation_amended_elsewhere(
        state.next_event_id(),
        v.value.patient_id,
        v.value.id,
        auth.user_id(),
    );
    let _ = state.events.send(ev);
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn list_by_patient(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Vec<Versioned<p::Observation>>>, ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    let rows = observation::list_by_patient(&state.db, auth.user_id(), pid)?;
    Ok(Json(
        rows.into_iter()
            .map(|v| Versioned::new(v.value.into(), v.version))
            .collect(),
    ))
}

async fn problem_list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Vec<Versioned<p::Observation>>>, ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    let rows = observation::problem_list(&state.db, auth.user_id(), pid)?;
    Ok(Json(
        rows.into_iter()
            .map(|v| Versioned::new(v.value.into(), v.version))
            .collect(),
    ))
}
