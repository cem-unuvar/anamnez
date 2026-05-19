//! `HttpTransport` trait — the seam between the view state machine and actual byte
//! movement. Two endpoint shapes: `EnrollEndpoint` (pre-enrollment; no mTLS identity)
//! and `ConnectedEndpoint` (post-enrollment; full mTLS).

use anamnez_protocol::auth::{LoginRequest, LoginResponse, RefreshRequest, RefreshResponse};
use anamnez_protocol::codesystem::{CodeSystem, SearchResponse};
use anamnez_protocol::encounter::{
    Encounter, FinishEncounterRequest, StartEncounterRequest,
};
use anamnez_protocol::enroll::{EnrollExchangeRequest, EnrollExchangeResponse};
use anamnez_protocol::health::HealthEnvelope;
use anamnez_protocol::ids::{EncounterId, ObservationId};
use anamnez_protocol::observation::{
    AmendObservationRequest, MarkEnteredInErrorRequest, NewObservation, Observation,
};
use anamnez_protocol::patient::{PatientDetail, PatientListQuery, PatientListResponse};
use anamnez_protocol::versioned::Versioned;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::ClientError;

/// Endpoint reachable without a client cert. Used for `health` and `enroll_exchange`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollEndpoint {
    /// LAN-side base URL, e.g. `https://10.0.0.5:8443`.
    pub base_url: String,
    /// Hex-encoded SHA-256 fingerprint of the server's TLS leaf cert. Pinned at
    /// the rustls verifier — no system trust store consulted.
    pub server_fingerprint_sha256: String,
    /// Workstation client version (`X-Client-Version` header).
    pub client_version: String,
}

/// Endpoint with full mTLS identity. Used for every authenticated route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedEndpoint {
    pub base_url: String,
    pub server_fingerprint_sha256: String,
    pub ca_cert_pem: String,
    pub client_cert_pem: String,
    pub client_key_pem: String,
    pub client_version: String,
}

impl ConnectedEndpoint {
    /// Downgrade to an enrollment-shape endpoint (drops the identity). Useful when
    /// hitting an unauthenticated route after enrollment is complete.
    #[must_use]
    pub fn as_enroll(&self) -> EnrollEndpoint {
        EnrollEndpoint {
            base_url: self.base_url.clone(),
            server_fingerprint_sha256: self.server_fingerprint_sha256.clone(),
            client_version: self.client_version.clone(),
        }
    }
}

/// On wasm32, futures don't need `Send` (single-threaded JS runtime); on native they
/// must be `Send` so the Tauri shell can drive them on tokio's multi-threaded runtime.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait HttpTransport: Send + Sync {
    /// `/v1/health` requires mTLS — only callable once the workstation has its
    /// device credential. Pre-enrollment, the bootstrap UI doesn't show a TEST shield.
    async fn health(&self, ep: &ConnectedEndpoint) -> Result<HealthEnvelope, ClientError>;

    async fn enroll_exchange(
        &self,
        ep: &EnrollEndpoint,
        req: EnrollExchangeRequest,
    ) -> Result<EnrollExchangeResponse, ClientError>;

    async fn login(
        &self,
        ep: &ConnectedEndpoint,
        req: LoginRequest,
    ) -> Result<LoginResponse, ClientError>;

    async fn refresh(
        &self,
        ep: &ConnectedEndpoint,
        req: RefreshRequest,
    ) -> Result<RefreshResponse, ClientError>;

    async fn logout(&self, ep: &ConnectedEndpoint, access_token: &str) -> Result<(), ClientError>;

    async fn list_patients(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        query: PatientListQuery,
    ) -> Result<PatientListResponse, ClientError>;

    async fn get_patient_detail(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        patient_id: anamnez_protocol::ids::PatientId,
    ) -> Result<PatientDetail, ClientError>;

    async fn search_codes(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        system: CodeSystem,
        q: String,
        limit: Option<usize>,
    ) -> Result<SearchResponse, ClientError>;

    async fn start_encounter(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        req: StartEncounterRequest,
    ) -> Result<Versioned<Encounter>, ClientError>;

    async fn finish_encounter(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        encounter_id: EncounterId,
        req: FinishEncounterRequest,
    ) -> Result<Versioned<Encounter>, ClientError>;

    async fn create_observation(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        req: NewObservation,
    ) -> Result<Versioned<Observation>, ClientError>;

    async fn amend_observation(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        observation_id: ObservationId,
        req: AmendObservationRequest,
    ) -> Result<Versioned<Observation>, ClientError>;

    async fn mark_observation_entered_in_error(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        observation_id: ObservationId,
        req: MarkEnteredInErrorRequest,
    ) -> Result<Versioned<Observation>, ClientError>;
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait HttpTransport {
    async fn health(&self, ep: &ConnectedEndpoint) -> Result<HealthEnvelope, ClientError>;

    async fn enroll_exchange(
        &self,
        ep: &EnrollEndpoint,
        req: EnrollExchangeRequest,
    ) -> Result<EnrollExchangeResponse, ClientError>;

    async fn login(
        &self,
        ep: &ConnectedEndpoint,
        req: LoginRequest,
    ) -> Result<LoginResponse, ClientError>;

    async fn refresh(
        &self,
        ep: &ConnectedEndpoint,
        req: RefreshRequest,
    ) -> Result<RefreshResponse, ClientError>;

    async fn logout(&self, ep: &ConnectedEndpoint, access_token: &str) -> Result<(), ClientError>;

    async fn list_patients(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        query: PatientListQuery,
    ) -> Result<PatientListResponse, ClientError>;

    async fn get_patient_detail(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        patient_id: anamnez_protocol::ids::PatientId,
    ) -> Result<PatientDetail, ClientError>;

    async fn search_codes(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        system: CodeSystem,
        q: String,
        limit: Option<usize>,
    ) -> Result<SearchResponse, ClientError>;

    async fn start_encounter(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        req: StartEncounterRequest,
    ) -> Result<Versioned<Encounter>, ClientError>;

    async fn finish_encounter(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        encounter_id: EncounterId,
        req: FinishEncounterRequest,
    ) -> Result<Versioned<Encounter>, ClientError>;

    async fn create_observation(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        req: NewObservation,
    ) -> Result<Versioned<Observation>, ClientError>;

    async fn amend_observation(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        observation_id: ObservationId,
        req: AmendObservationRequest,
    ) -> Result<Versioned<Observation>, ClientError>;

    async fn mark_observation_entered_in_error(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        observation_id: ObservationId,
        req: MarkEnteredInErrorRequest,
    ) -> Result<Versioned<Observation>, ClientError>;
}
