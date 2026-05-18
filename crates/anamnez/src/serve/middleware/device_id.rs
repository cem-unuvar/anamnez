//! Asserts that the TLS handshake produced a `WorkstationId` (i.e., the client
//! presented a valid, non-revoked client cert). Reject otherwise.
//!
//! Applied to every route group **except** the enrollment exchange route — see
//! `serve/routes/mod.rs::build`.

use crate::serve::error::ApiError;
use anamnez_core::error::Error;
use anamnez_core::ids::WorkstationId;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::Extension;

pub async fn require_device_id(
    device: Option<Extension<WorkstationId>>,
    req: Request,
    next: Next,
) -> std::result::Result<Response, ApiError> {
    let _ = device.ok_or(ApiError(Error::Forbidden))?;
    Ok(next.run(req).await)
}
