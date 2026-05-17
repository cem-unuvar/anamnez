//! /v1/patients/:id/access ... + dynamic step-up gate for grant-to-new-user.

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::middleware::stepup::require_stepup;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::auth::stepup::StepUpAction;
use anamnez_core::ids::{PatientId, UserId};
use anamnez_core::patient_access;
use anamnez_protocol::access as p;
use anamnez_protocol::events::ServerEvent;
use axum::extract::{Extension, Path, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/patients/:id/access", get(list).post(grant))
        .route("/v1/patients/:id/access/:user_id", delete(revoke))
        .route(
            "/v1/patients/:id/transfer-ownership",
            post(transfer_ownership),
        )
}

async fn list(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Vec<p::PatientAccess>>, ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    let rows = patient_access::list(&state.db, pid)?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn grant(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    Json(req): Json<p::GrantAccessRequest>,
) -> std::result::Result<(), ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    let grantee: UserId = req.user_id.into();
    // Dynamic step-up gate: if the grantee has no prior access, this is a
    // "grant-to-new-user" and requires step-up reauth.
    let prior = patient_access::level_for(&state.db, grantee, pid)?;
    if prior.is_none() {
        require_stepup(
            &state,
            auth.user_id(),
            StepUpAction::PatientAccessGrantToNewUser,
            &headers,
        )?;
    }
    patient_access::grant(&state.db, auth.user_id(), pid, grantee, req.level.into())?;
    let ev = ServerEvent::patient_access_changed(
        state.next_event_id(),
        pid,
        grantee,
        Some(anamnez_core::patient_access::AccessLevel::from(req.level)),
    );
    let _ = state.events.send(ev);
    Ok(())
}

async fn revoke(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path((id_str, user_str)): Path<(String, String)>,
) -> std::result::Result<(), ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    let uid = UserId(parse_uuid(&user_str)?);
    patient_access::revoke(&state.db, auth.user_id(), pid, uid)?;
    let ev = ServerEvent::patient_access_changed(state.next_event_id(), pid, uid, None);
    let _ = state.events.send(ev);
    Ok(())
}

async fn transfer_ownership(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::TransferOwnershipRequest>,
) -> std::result::Result<(), ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    patient_access::transfer_ownership(
        &state.db,
        auth.user_id(),
        pid,
        req.from.into(),
        req.to.into(),
    )?;
    Ok(())
}
