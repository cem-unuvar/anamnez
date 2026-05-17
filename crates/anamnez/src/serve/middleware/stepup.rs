//! Step-up reauthentication helper. Called inline from protected handlers because
//! each route names its own `StepUpAction` constant.

use crate::serve::app_state::AppState;
use anamnez_core::auth::stepup::{verify_for, StepUpAction, StepUpReceipt};
use anamnez_core::error::{Error, Result};
use anamnez_core::ids::UserId;
use axum::http::HeaderMap;
use secrecy::SecretString;

const HEADER: &str = "x-step-up-password";

pub fn require_stepup(
    state: &AppState,
    user_id: UserId,
    action: StepUpAction,
    headers: &HeaderMap,
) -> Result<StepUpReceipt> {
    let pw = headers
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(Error::StepUpRequired {
            action: action.as_str(),
        })?;
    verify_for(
        &state.db,
        user_id,
        action,
        SecretString::from(pw.to_owned()),
    )
}
