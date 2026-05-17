use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::consent;
use anamnez_core::ids::PatientConsentId;
use anamnez_protocol::consent as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, State};
use axum::routing::post;
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/consents", post(record))
        .route("/v1/consents/:id/revoke", post(revoke))
}

async fn record(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<p::RecordConsentRequest>,
) -> std::result::Result<Json<Versioned<p::PatientConsent>>, ApiError> {
    let v = consent::record(
        &state.db,
        auth.user_id(),
        req.patient_id.into(),
        req.purpose.into(),
        req.evidence_source_id.map(Into::into),
        req.notes,
    )?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn revoke(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::RevokeConsentRequest>,
) -> std::result::Result<Json<Versioned<p::PatientConsent>>, ApiError> {
    let id = PatientConsentId(parse_uuid(&id_str)?);
    let v = consent::revoke(&state.db, auth.user_id(), id, req.expected_version)?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}
