use crate::serve::app_state::AppState;
use crate::serve::sse;
use axum::routing::get;
use axum::Router;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/events", get(sse::events))
}
