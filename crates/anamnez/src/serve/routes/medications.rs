use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::ids::MedicationId;
use anamnez_core::medication;
use anamnez_protocol::medication as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, State};
use axum::routing::{patch, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/medications", post(create))
        .route("/v1/medications/:id", patch(amend))
}

async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(input): Json<p::NewMedication>,
) -> std::result::Result<Json<Versioned<p::Medication>>, ApiError> {
    let v = medication::create(&state.db, auth.user_id(), input.into())?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn amend(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::AmendMedicationRequest>,
) -> std::result::Result<Json<Versioned<p::Medication>>, ApiError> {
    let id = MedicationId(parse_uuid(&id_str)?);
    let v = medication::amend(
        &state.db,
        auth.user_id(),
        id,
        req.expected_version,
        req.patch.into(),
    )?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}
