//! `POST /v1/enroll/exchange` — token-authenticated, no mTLS, no Bearer.
//!
//! Reachable on a TLS connection that does not present a client cert (see
//! `serve::mtls::AnamnezClientVerifier::client_auth_mandatory` returning false and
//! `serve::tls_serve::serve_connection` making the device-id extension optional).
//! Every other route lives behind the `require_device_id` middleware in
//! `serve::routes::mod::build` and is unreachable without a client cert.

use crate::serve::app_state::AppState;
use crate::serve::error::ApiError;
use anamnez_core::error::Error;
use anamnez_core::workstation;
use anamnez_protocol::enroll as p;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use secrecy::SecretString;

pub fn router_no_mtls() -> Router<AppState> {
    Router::new().route("/v1/enroll/exchange", post(exchange))
}

async fn exchange(
    State(state): State<AppState>,
    Json(req): Json<p::EnrollExchangeRequest>,
) -> std::result::Result<Json<p::EnrollExchangeResponse>, ApiError> {
    let data_dir = state
        .config
        .db_path
        .parent()
        .ok_or(ApiError(Error::Invariant("db_path has no parent")))?;
    let token = SecretString::from(req.token);
    let exchanged = workstation::exchange_enrollment(&state.db, data_dir, &token)?;
    Ok(Json(p::EnrollExchangeResponse {
        workstation_id: exchanged.workstation_id.into(),
        client_cert_pem: exchanged.client_cert_pem,
        client_key_pem: exchanged.client_key_pem,
        ca_cert_pem: exchanged.ca_cert_pem,
    }))
}
