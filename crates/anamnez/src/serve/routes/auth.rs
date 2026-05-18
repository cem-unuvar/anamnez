//! /v1/auth/{login,refresh,logout}

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use anamnez_core::auth as core_auth;
use anamnez_protocol::auth as p;
use axum::extract::{Extension, State};
use axum::routing::post;
use axum::{Json, Router};
use secrecy::{ExposeSecret, SecretString};

pub fn router_unauthed() -> Router<AppState> {
    Router::new()
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/refresh", post(refresh))
}

pub fn router_authed() -> Router<AppState> {
    Router::new().route("/v1/auth/logout", post(logout))
}

async fn login(
    State(state): State<AppState>,
    Extension(device_id): Extension<anamnez_core::ids::WorkstationId>,
    Json(req): Json<p::LoginRequest>,
) -> std::result::Result<Json<p::LoginResponse>, ApiError> {
    let outcome = core_auth::login(
        &state.db,
        &req.email,
        SecretString::from(req.password),
        device_id,
    )?;
    Ok(Json(p::LoginResponse {
        user: outcome.user.into(),
        access_token: outcome.access_token.expose_secret().to_owned(),
        refresh_token: outcome.refresh_token.expose_secret().to_owned(),
        environment: state.config.environment.into(),
        idle_lock_minutes: state.config.idle_lock_minutes,
    }))
}

async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<p::RefreshRequest>,
) -> std::result::Result<Json<p::RefreshResponse>, ApiError> {
    let outcome = core_auth::refresh(&state.db, SecretString::from(req.refresh_token))?;
    Ok(Json(p::RefreshResponse {
        access_token: outcome.access_token.expose_secret().to_owned(),
        refresh_token: outcome.refresh_token.expose_secret().to_owned(),
    }))
}

async fn logout(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> std::result::Result<(), ApiError> {
    core_auth::revoke(&state.db, auth.auth_session_id)?;
    Ok(())
}
