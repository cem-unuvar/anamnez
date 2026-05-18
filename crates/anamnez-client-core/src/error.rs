//! Client-side error type. The native transport maps:
//! - Transport-level failures (TLS, DNS, refused) → `ClientError::Transport`.
//! - 4xx/5xx with a parseable `ErrorEnvelope` → `ClientError::Server(ErrorEnvelope)`.
//! - Other HTTP failures → `ClientError::HttpStatus`.
//!
//! The WASM/Tauri-invoke transport forwards already-encoded `ErrorEnvelope` values, so
//! the WASM side only ever sees `ClientError::Server` or `ClientError::Transport`.

use anamnez_protocol::error::ErrorEnvelope;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    /// The server returned a structured `ErrorEnvelope`.
    #[error("server error: {0}")]
    Server(#[from] ErrorEnvelope),
    /// Network-level failure — DNS, TLS handshake, fingerprint mismatch, TCP RST,
    /// timeout. Drives the workstation's "Disconnected" banner.
    #[error("transport: {0}")]
    Transport(String),
    /// HTTP error response with a non-`ErrorEnvelope` body (very rare; daemon should
    /// always emit the envelope, but we keep the variant for forward compat).
    #[error("http {status}: {body}")]
    HttpStatus { status: u16, body: String },
    /// Local serialization / deserialization issue.
    #[error("serde: {0}")]
    Serde(String),
    /// IPC failure (wasm32-only): the Tauri `invoke` call failed before reaching native.
    #[error("ipc: {0}")]
    Ipc(String),
}

impl ClientError {
    /// True for `ErrorEnvelope::SessionExpired` / `Revoked` / `BadCredentials` — the
    /// workstation should clear its session and return to the login screen.
    #[must_use]
    pub fn is_auth_lost(&self) -> bool {
        matches!(
            self,
            ClientError::Server(ErrorEnvelope::SessionExpired)
                | ClientError::Server(ErrorEnvelope::Revoked)
                | ClientError::Server(ErrorEnvelope::BadCredentials)
        )
    }

    /// True for transport-level failures — flips the "Disconnected" banner on.
    #[must_use]
    pub fn is_transport(&self) -> bool {
        matches!(self, ClientError::Transport(_))
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serde(e.to_string())
    }
}
