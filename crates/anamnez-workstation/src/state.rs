//! Shared app state held inside `tauri::State`. Owns the transport, the optional
//! connected endpoint, and the in-memory access token. None of the clinical data
//! lives here (per SPEC: "No clinical data, ever, on workstation disk").

use std::sync::Arc;

use anamnez_client_core::transport::ConnectedEndpoint;
use anamnez_client_core::transport_native::NativeTransport;
use parking_lot::RwLock;

pub struct AppState {
    pub transport: Arc<NativeTransport>,
    pub connected: RwLock<Option<ConnectedEndpoint>>,
    /// In-process only — never written to disk. Cleared by `seal_session` (idle
    /// lock) and `logout`.
    pub access_token: RwLock<Option<String>>,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            transport: Arc::new(NativeTransport::new()),
            connected: RwLock::new(None),
            access_token: RwLock::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
