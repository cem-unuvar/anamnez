use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::allergy;
use anamnez_core::ids::{AllergyId, PatientId};
use anamnez_protocol::allergy as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/allergies", post(create))
        .route("/v1/allergies/:id", axum::routing::patch(amend))
        .route("/v1/patients/:id/allergies", get(list_by_patient))
}

async fn list_by_patient(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Vec<Versioned<p::Allergy>>>, ApiError> {
    let pid = PatientId(parse_uuid(&id_str)?);
    let rows = allergy::list_by_patient(&state.db, auth.user_id(), pid)?;
    Ok(Json(
        rows.into_iter()
            .map(|v| Versioned::new(v.value.into(), v.version))
            .collect(),
    ))
}

async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(input): Json<p::NewAllergy>,
) -> std::result::Result<Json<Versioned<p::Allergy>>, ApiError> {
    let v = allergy::create(&state.db, auth.user_id(), input.into())?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn amend(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::AmendAllergyRequest>,
) -> std::result::Result<Json<Versioned<p::Allergy>>, ApiError> {
    let id = AllergyId(parse_uuid(&id_str)?);
    let v = allergy::amend(
        &state.db,
        auth.user_id(),
        id,
        req.expected_version,
        req.patch.into(),
    )?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}
