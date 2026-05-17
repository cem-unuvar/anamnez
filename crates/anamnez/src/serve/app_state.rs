//! Per-process state shared with every handler via axum `State`.

use anamnez_core::auth::User;
use anamnez_core::config::Config;
use anamnez_core::db::Database;
use anamnez_core::ids::{AuthSessionId, UserId, WorkstationId};
use anamnez_protocol::events::ServerEvent;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub events: broadcast::Sender<ServerEvent>,
    pub revoked_devices: Arc<RwLock<HashSet<WorkstationId>>>,
    pub config: Arc<Config>,
    pub event_counter: Arc<AtomicU64>,
}

impl AppState {
    pub fn next_event_id(&self) -> u64 {
        self.event_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

/// Carried in `axum::Extension` on every authenticated request.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: User,
    pub auth_session_id: AuthSessionId,
    /// Held for traceability / future per-device policy hooks. Authz logic
    /// already cross-checks the mTLS device against the session at middleware time.
    #[allow(dead_code)]
    pub device_id: WorkstationId,
}

impl AuthContext {
    #[must_use]
    pub fn user_id(&self) -> UserId {
        self.user.id
    }
}
