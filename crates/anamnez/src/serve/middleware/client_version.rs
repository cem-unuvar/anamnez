//! `X-Client-Version` floor check. Applies to **every** request (auth and unauth).

use crate::serve::app_state::AppState;
use crate::serve::error::ApiError;
use anamnez_core::error::Error;
use anamnez_core::wire::predicates::check_client_version;
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;

const HEADER: &str = "x-client-version";

pub async fn require_client_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
    next: Next,
) -> std::result::Result<Response, ApiError> {
    let v = headers
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            ApiError(Error::OutdatedClient {
                min: state.config.min_client_version.display(),
                got: "<absent>".to_owned(),
            })
        })?;
    check_client_version(&state.config.min_client_version, v)?;
    Ok(next.run(req).await)
}
