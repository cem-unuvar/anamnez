//! Active-session state. Owned by whoever drives the workstation (Tauri shell native
//! side, or the WASM UI when it caches what came back from `bootstrap_state`).

use anamnez_protocol::auth::{LoginResponse, User};
use anamnez_protocol::environment::Environment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub user: User,
    pub access_token: String,
    /// The refresh token persisted in the OS secret store. Held in-memory here only
    /// transiently between mint and write — never serialized into config or logged.
    pub refresh_token: String,
    pub environment: Environment,
    pub idle_lock_minutes: u32,
}

impl Session {
    #[must_use]
    pub fn from_login(resp: LoginResponse) -> Self {
        Self {
            user: resp.user,
            access_token: resp.access_token,
            refresh_token: resp.refresh_token,
            environment: resp.environment,
            idle_lock_minutes: resp.idle_lock_minutes,
        }
    }

    /// Update the access + refresh tokens after a refresh round-trip. Other fields are
    /// unchanged — environment and policy come from login only.
    pub fn rotate_tokens(&mut self, access: String, refresh: String) {
        self.access_token = access;
        self.refresh_token = refresh;
    }
}
