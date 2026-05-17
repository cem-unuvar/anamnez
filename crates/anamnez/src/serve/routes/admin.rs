//! Admin routes: user lifecycle, workstation enrollment / revocation, breach report, exports.

use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::middleware::stepup::require_stepup;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::auth::stepup::StepUpAction;
use anamnez_core::auth::UserRole;
use anamnez_core::ids::{PatientId, UserId, WorkstationId};
use anamnez_core::{kvkk, user, workstation};
use anamnez_protocol::events::ServerEvent;
use axum::extract::{Extension, Path, State};
use axum::http::HeaderMap;
use axum::routing::{patch, post};
use axum::{Json, Router};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/admin/users", post(create_user))
        .route("/v1/admin/users/:id", patch(update_user))
        .route("/v1/admin/users/:id/disable", post(disable_user))
        .route("/v1/admin/workstations", post(enroll_workstation))
        .route(
            "/v1/admin/workstations/:id/revoke",
            post(revoke_workstation),
        )
}

fn require_admin(auth: &AuthContext) -> std::result::Result<(), ApiError> {
    if matches!(auth.user.role, UserRole::Admin) {
        Ok(())
    } else {
        Err(ApiError(anamnez_core::Error::Forbidden))
    }
}

#[derive(Debug, Deserialize)]
struct CreateUserBody {
    email: String,
    display_name: String,
    role: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct UserOut {
    id: anamnez_protocol::ids::UserId,
    email: String,
    display_name: String,
    role: String,
}

async fn create_user(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    headers: HeaderMap,
    Json(body): Json<CreateUserBody>,
) -> std::result::Result<Json<UserOut>, ApiError> {
    require_admin(&auth)?;
    require_stepup(&state, auth.user_id(), StepUpAction::UserCreate, &headers)?;
    let role = UserRole::parse(&body.role)?;
    let u = user::create(
        &state.db,
        auth.user_id(),
        anamnez_core::user::NewUser {
            email: body.email,
            display_name: body.display_name,
            role,
            password: SecretString::from(body.password),
        },
    )?;
    Ok(Json(UserOut {
        id: u.id.into(),
        email: u.email,
        display_name: u.display_name,
        role: u.role.as_str().to_owned(),
    }))
}

#[derive(Debug, Deserialize)]
struct UpdateUserBody {
    display_name: Option<String>,
    role: Option<String>,
}

async fn update_user(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateUserBody>,
) -> std::result::Result<Json<UserOut>, ApiError> {
    require_admin(&auth)?;
    require_stepup(&state, auth.user_id(), StepUpAction::UserModify, &headers)?;
    let uid = UserId(parse_uuid(&id_str)?);
    let patch = anamnez_core::user::UserPatch {
        display_name: body.display_name,
        role: match body.role {
            None => None,
            Some(s) => Some(UserRole::parse(&s)?),
        },
        disabled_at: None,
    };
    let u = user::update(&state.db, auth.user_id(), uid, patch)?;
    Ok(Json(UserOut {
        id: u.id.into(),
        email: u.email,
        display_name: u.display_name,
        role: u.role.as_str().to_owned(),
    }))
}

#[derive(Debug, Deserialize)]
struct DisableUserBody {
    successors: Vec<DisableSuccessor>,
}
#[derive(Debug, Deserialize)]
struct DisableSuccessor {
    patient_id: anamnez_protocol::ids::PatientId,
    user_id: anamnez_protocol::ids::UserId,
}

async fn disable_user(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    Json(body): Json<DisableUserBody>,
) -> std::result::Result<(), ApiError> {
    require_admin(&auth)?;
    require_stepup(&state, auth.user_id(), StepUpAction::UserDisable, &headers)?;
    let target = UserId(parse_uuid(&id_str)?);
    let successors: Vec<(PatientId, UserId)> = body
        .successors
        .into_iter()
        .map(|s| (PatientId::from(s.patient_id), UserId::from(s.user_id)))
        .collect();
    kvkk::ownership_transfer::disable_user_with_successors(
        &state.db,
        auth.user_id(),
        target,
        successors,
    )?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct EnrollWorkstationBody {
    label: String,
    mode: String, // "bound" | "shared"
    bound_user_id: Option<anamnez_protocol::ids::UserId>,
    cert_serial: String,
    cert_fingerprint: String,
}
#[derive(Debug, Serialize)]
struct EnrollWorkstationOut {
    id: anamnez_protocol::ids::WorkstationId,
}

async fn enroll_workstation(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    headers: HeaderMap,
    Json(body): Json<EnrollWorkstationBody>,
) -> std::result::Result<Json<EnrollWorkstationOut>, ApiError> {
    require_admin(&auth)?;
    require_stepup(
        &state,
        auth.user_id(),
        StepUpAction::WorkstationEnrollmentString,
        &headers,
    )?;
    let mode = workstation::Mode::parse(&body.mode)?;
    let ws = workstation::enroll(
        &state.db,
        auth.user_id(),
        workstation::NewWorkstation {
            label: body.label,
            mode,
            bound_user_id: body.bound_user_id.map(Into::into),
            cert_serial: body.cert_serial,
            cert_fingerprint: body.cert_fingerprint,
        },
    )?;
    Ok(Json(EnrollWorkstationOut { id: ws.id.into() }))
}

#[derive(Debug, Deserialize)]
struct RevokeWorkstationBody {
    reason: String,
}

async fn revoke_workstation(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    Json(body): Json<RevokeWorkstationBody>,
) -> std::result::Result<(), ApiError> {
    require_admin(&auth)?;
    require_stepup(
        &state,
        auth.user_id(),
        StepUpAction::WorkstationRevoke,
        &headers,
    )?;
    let wid = WorkstationId(parse_uuid(&id_str)?);
    workstation::revoke(&state.db, auth.user_id(), wid, body.reason)?;
    state.revoked_devices.write().insert(wid);
    // Fan out ForcedLogout to every live session on the revoked device.
    for sid in workstation::list_sessions_on(&state.db, wid)? {
        let _ = state.events.send(ServerEvent::forced_logout(
            state.next_event_id(),
            format!(
                "workstation {} revoked (session {})",
                wid.as_uuid(),
                sid.as_uuid()
            ),
        ));
    }
    Ok(())
}
