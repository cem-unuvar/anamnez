//! Newtype wrapper around `anamnez_core::Error` + IntoResponse mapping.

use anamnez_core::Error as CoreError;
use anamnez_protocol::error::ErrorEnvelope;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

/// Adapter type. Orphan rule forbids implementing `IntoResponse` for `CoreError`
/// directly (foreign trait on foreign type), so the daemon wraps it.
pub struct ApiError(pub CoreError);

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = status_for(&self.0);
        if matches!(status, StatusCode::INTERNAL_SERVER_ERROR) {
            tracing::error!(error = ?self.0, "internal error");
        } else {
            tracing::debug!(error = ?self.0, "client error");
        }
        let envelope: ErrorEnvelope = (&self.0).into();
        (status, Json(envelope)).into_response()
    }
}

fn status_for(e: &CoreError) -> StatusCode {
    match e {
        CoreError::Conflict { .. } => StatusCode::CONFLICT,
        CoreError::NotFound => StatusCode::NOT_FOUND,
        CoreError::Forbidden => StatusCode::FORBIDDEN,
        CoreError::BadCredentials
        | CoreError::Revoked
        | CoreError::SessionExpired
        | CoreError::StepUpRequired { .. } => StatusCode::UNAUTHORIZED,
        CoreError::OutdatedClient { .. } => StatusCode::UPGRADE_REQUIRED,
        CoreError::CodeSystemNotAllowed { .. }
        | CoreError::CodeSystemMismatch { .. }
        | CoreError::RetiredCode { .. }
        | CoreError::InvalidStateTransition { .. }
        | CoreError::TestPrefixRequired
        | CoreError::SoleOwnerOfPatient { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::AuditTamper { .. }
        | CoreError::Invariant(_)
        | CoreError::Db(_)
        | CoreError::Io(_)
        | CoreError::Serde(_)
        | CoreError::Csv(_)
        | CoreError::EnvironmentMarkerMismatch { .. }
        | CoreError::SchemaVersionMismatch { .. }
        | CoreError::InvalidBundleSignature => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
