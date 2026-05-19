//! Route module roots — `build(state)` assembles the full router.

pub mod admin;
pub mod allergies;
pub mod analysis;
pub mod auth;
pub mod codesystems;
pub mod consents;
pub mod encounters;
pub mod enroll;
pub mod events;
pub mod health;
pub mod medications;
pub mod observations;
pub mod patient_access;
pub mod patients;
pub mod source_documents;

use crate::serve::app_state::AppState;
use crate::serve::middleware;
use axum::middleware as ax_mw;
use axum::Router;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

const MAX_BODY_BYTES: usize = 50 * 1024 * 1024;

pub fn build(state: AppState) -> Router {
    // Reachable without a client cert. Token-authenticated only. Lives outside
    // `require_device_id` because the workstation has no client cert at enrollment time.
    let no_mtls = enroll::router_no_mtls();

    // Unauthenticated routes (login/refresh/health) — Bearer is not required, but a
    // client cert (and therefore `Extension<WorkstationId>`) is.
    let unauthed = Router::new()
        .merge(auth::router_unauthed())
        .merge(health::router());

    let authed = Router::new()
        .merge(auth::router_authed())
        .merge(patients::router())
        .merge(observations::router())
        .merge(encounters::router())
        .merge(allergies::router())
        .merge(medications::router())
        .merge(source_documents::router())
        .merge(consents::router())
        .merge(patient_access::router())
        .merge(analysis::router())
        .merge(codesystems::router())
        .merge(events::router())
        .merge(admin::router())
        .layer(ax_mw::from_fn_with_state(
            state.clone(),
            middleware::auth::require_auth,
        ));

    let with_device = Router::new()
        .merge(unauthed)
        .merge(authed)
        .layer(ax_mw::from_fn(middleware::device_id::require_device_id));

    Router::new()
        .merge(no_mtls)
        .merge(with_device)
        .layer(ax_mw::from_fn_with_state(
            state.clone(),
            middleware::client_version::require_client_version,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .with_state(state)
}
