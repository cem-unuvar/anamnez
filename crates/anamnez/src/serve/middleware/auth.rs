//! Bearer-token auth middleware. Populates `Extension<AuthContext>` on success.

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use anamnez_core::auth::check_session_with_id;
use anamnez_core::error::Error;
use anamnez_core::ids::WorkstationId;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::Extension;
use axum_extra::headers::authorization::Bearer;
use axum_extra::headers::Authorization;
use axum_extra::TypedHeader;
use secrecy::SecretString;

pub async fn require_auth(
    State(state): State<AppState>,
    Extension(device_id): Extension<WorkstationId>,
    bearer: Option<TypedHeader<Authorization<Bearer>>>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, ApiError> {
    let TypedHeader(Authorization(bearer)) =
        bearer.ok_or_else(|| ApiError(Error::BadCredentials))?;
    let token = SecretString::from(bearer.token().to_owned());
    let (user, session_id, session_device_id) = check_session_with_id(&state.db, &token)?;
    // Cross-check: the mTLS-presented device must match the session's bound device.
    if session_device_id != device_id {
        return Err(ApiError(Error::Forbidden));
    }
    req.extensions_mut().insert(AuthContext {
        user,
        auth_session_id: session_id,
        device_id,
    });
    Ok(next.run(req).await)
}
