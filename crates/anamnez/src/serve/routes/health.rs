use crate::serve::app_state::AppState;
use anamnez_protocol::health::HealthEnvelope;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/health", get(health))
}

async fn health(State(state): State<AppState>) -> Json<HealthEnvelope> {
    Json(HealthEnvelope {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        environment: state.config.environment.into(),
    })
}
