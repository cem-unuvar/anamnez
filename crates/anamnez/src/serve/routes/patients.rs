//! /v1/patients[/...]

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use anamnez_core::ids::PatientId;
use anamnez_core::{allergy, encounter, medication, observation, patient};
use anamnez_protocol::access::AccessLevel;
use anamnez_protocol::patient as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/patients", post(create).get(list))
        .route("/v1/patients/:id", get(get_one).patch(update))
        .route("/v1/patients/:id/detail", get(detail))
}

async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(input): Json<p::NewPatient>,
) -> std::result::Result<Json<Versioned<p::Patient>>, ApiError> {
    let v = patient::create(&state.db, auth.user_id(), input.into())?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn get_one(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Versioned<p::Patient>>, ApiError> {
    let id = PatientId(parse_uuid(&id_str)?);
    let v = patient::get(&state.db, auth.user_id(), id)?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    Json(req): Json<p::UpdatePatientRequest>,
) -> std::result::Result<Json<Versioned<p::Patient>>, ApiError> {
    let id = PatientId(parse_uuid(&id_str)?);
    let v = patient::update(
        &state.db,
        auth.user_id(),
        id,
        req.expected_version,
        req.patch.into(),
    )?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Query(q): Query<p::PatientListQuery>,
) -> std::result::Result<Json<p::PatientListResponse>, ApiError> {
    let rows = patient::list_for_user(
        &state.db,
        auth.user_id(),
        anamnez_core::patient::PatientListQuery {
            q: q.q,
            include_archived: q.include_archived,
            limit: q.limit,
        },
    )?;
    let items: Vec<p::PatientListItem> = rows
        .into_iter()
        .map(|r| p::PatientListItem {
            id: r.id.into(),
            mrn: r.mrn,
            given_names: r.given_names,
            family_name: r.family_name,
            preferred_name: r.preferred_name,
            date_of_birth: r.date_of_birth,
            sex_assigned_at_birth: r.sex_assigned_at_birth.into(),
            access_level: r.access_level.into(),
            updated_at: r.updated_at,
            deceased_at: r.deceased_at,
            archived_at: r.archived_at,
        })
        .collect();
    Ok(Json(p::PatientListResponse {
        items,
        next_before: None,
    }))
}

async fn detail(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<p::PatientDetail>, ApiError> {
    let id = PatientId(parse_uuid(&id_str)?);
    // `patient::get` already checks access; if the caller has none, NotFound.
    let v = patient::get(&state.db, auth.user_id(), id)?;
    let access_level = anamnez_core::patient_access::level_for(&state.db, auth.user_id(), id)?
        .ok_or(ApiError(anamnez_core::Error::NotFound))?;
    let problem_list = observation::problem_list(&state.db, auth.user_id(), id)?;
    let allergies = allergy::list_by_patient(&state.db, auth.user_id(), id)?;
    let medications = medication::list_by_patient(&state.db, auth.user_id(), id)?;
    let encounters = encounter::list_by_patient(&state.db, auth.user_id(), id)?;
    let detail = p::PatientDetail {
        patient: v.value.into(),
        access_level: AccessLevel::from(access_level),
        problem_list: problem_list.into_iter().map(|v| v.value.into()).collect(),
        allergies: allergies.into_iter().map(|v| v.value.into()).collect(),
        medications: medications.into_iter().map(|v| v.value.into()).collect(),
        encounters: encounters.into_iter().map(|v| v.value.into()).collect(),
    };
    Ok(Json(detail))
}

pub(crate) fn parse_uuid(s: &str) -> std::result::Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(s).map_err(|_| ApiError(anamnez_core::Error::NotFound))
}
