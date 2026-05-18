//! `POST /v1/enroll/exchange` — the one route reachable without mTLS and without Bearer.
//!
//! The workstation arrives with only a one-time token (carried in the
//! `anamnez://enroll?token=…` URI minted by an admin) and the pinned server fingerprint.
//! It exchanges the token for a freshly-issued client cert + private key (signed by the
//! local CA) and the CA cert itself. All subsequent connections are mTLS.

use serde::{Deserialize, Serialize};

use crate::ids::WorkstationId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollExchangeRequest {
    /// Hex-encoded one-time token from the enrollment URI.
    pub token: String,
    /// Workstation client version (matches the `X-Client-Version` header value).
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollExchangeResponse {
    pub workstation_id: WorkstationId,
    pub client_cert_pem: String,
    pub client_key_pem: String,
    pub ca_cert_pem: String,
}
