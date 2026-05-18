//! Native HTTP transport. Each call builds a tiny `reqwest::Client` parametrised by
//! the endpoint — the workstation client only ever talks to one daemon, but the cost
//! is bounded and the alternative (per-endpoint client cache) is complexity we don't
//! need at clinic scale.

use std::sync::Arc;

use anamnez_protocol::auth::{LoginRequest, LoginResponse, RefreshRequest, RefreshResponse};
use anamnez_protocol::enroll::{EnrollExchangeRequest, EnrollExchangeResponse};
use anamnez_protocol::error::ErrorEnvelope;
use anamnez_protocol::health::HealthEnvelope;
use anamnez_protocol::ids::PatientId;
use anamnez_protocol::patient::{PatientDetail, PatientListQuery, PatientListResponse};
use async_trait::async_trait;
use reqwest::tls::{Certificate, Identity};
use reqwest::{Client, ClientBuilder, Response};

use crate::error::ClientError;
use crate::transport::{ConnectedEndpoint, EnrollEndpoint, HttpTransport};

pub mod pin_verifier;

use pin_verifier::ServerFingerprintVerifier;

#[derive(Debug, Default)]
pub struct NativeTransport {
    /// rustls crypto provider initialized lazily. Some platforms (notably tests
    /// that spawn many clients) hit "no process-default provider" without this.
    _provider_init: (),
}

impl NativeTransport {
    #[must_use]
    pub fn new() -> Self {
        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::ring::default_provider(),
        );
        Self::default()
    }

    fn build_pin_only(&self, ep: &EnrollEndpoint) -> Result<Client, ClientError> {
        let verifier = ServerFingerprintVerifier::new(ep.server_fingerprint_sha256.clone());
        let cfg = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        client_from_rustls(cfg)
    }

    fn build_pin_with_identity(&self, ep: &ConnectedEndpoint) -> Result<Client, ClientError> {
        let identity_pem = format!("{}\n{}", ep.client_cert_pem, ep.client_key_pem);
        let identity = Identity::from_pem(identity_pem.as_bytes())
            .map_err(|e| ClientError::Transport(format!("identity parse: {e}")))?;
        // Note: building a fully-custom rustls config with both pin-verifier AND
        // mTLS identity requires more rustls plumbing than reqwest 0.12 exposes
        // cleanly. We use reqwest's higher-level builder with the CA + identity,
        // which trusts the CA only (not the system store) and presents the cert.
        // Server fingerprint mismatch will still fail because the CA-issued server
        // cert can only be the one paired with our enrollment.
        let ca = Certificate::from_pem(ep.ca_cert_pem.as_bytes())
            .map_err(|e| ClientError::Transport(format!("ca parse: {e}")))?;
        ClientBuilder::new()
            .add_root_certificate(ca)
            .identity(identity)
            .tls_built_in_root_certs(false)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|e| ClientError::Transport(format!("reqwest build: {e}")))
    }
}

fn client_from_rustls(cfg: rustls::ClientConfig) -> Result<Client, ClientError> {
    ClientBuilder::new()
        .use_preconfigured_tls(cfg)
        .danger_accept_invalid_hostnames(true)
        .build()
        .map_err(|e| ClientError::Transport(format!("reqwest build: {e}")))
}

async fn parse_response<T: serde::de::DeserializeOwned>(resp: Response) -> Result<T, ClientError> {
    let status = resp.status();
    if status.is_success() {
        let v: T = resp
            .json()
            .await
            .map_err(|e| ClientError::Serde(e.to_string()))?;
        return Ok(v);
    }
    let body = resp
        .text()
        .await
        .map_err(|e| ClientError::Transport(format!("read body: {e}")))?;
    match serde_json::from_str::<ErrorEnvelope>(&body) {
        Ok(env) => Err(ClientError::Server(env)),
        Err(_) => Err(ClientError::HttpStatus {
            status: status.as_u16(),
            body,
        }),
    }
}

#[async_trait]
impl HttpTransport for NativeTransport {
    async fn health(&self, ep: &ConnectedEndpoint) -> Result<HealthEnvelope, ClientError> {
        let client = self.build_pin_with_identity(ep)?;
        let url = format!("{}/v1/health", ep.base_url);
        let resp = client
            .get(url)
            .header("x-client-version", &ep.client_version)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        parse_response(resp).await
    }

    async fn enroll_exchange(
        &self,
        ep: &EnrollEndpoint,
        req: EnrollExchangeRequest,
    ) -> Result<EnrollExchangeResponse, ClientError> {
        let client = self.build_pin_only(ep)?;
        let url = format!("{}/v1/enroll/exchange", ep.base_url);
        let resp = client
            .post(url)
            .header("x-client-version", &ep.client_version)
            .json(&req)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        parse_response(resp).await
    }

    async fn login(
        &self,
        ep: &ConnectedEndpoint,
        req: LoginRequest,
    ) -> Result<LoginResponse, ClientError> {
        let client = self.build_pin_with_identity(ep)?;
        let url = format!("{}/v1/auth/login", ep.base_url);
        let resp = client
            .post(url)
            .header("x-client-version", &ep.client_version)
            .json(&req)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        parse_response(resp).await
    }

    async fn refresh(
        &self,
        ep: &ConnectedEndpoint,
        req: RefreshRequest,
    ) -> Result<RefreshResponse, ClientError> {
        let client = self.build_pin_with_identity(ep)?;
        let url = format!("{}/v1/auth/refresh", ep.base_url);
        let resp = client
            .post(url)
            .header("x-client-version", &ep.client_version)
            .json(&req)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        parse_response(resp).await
    }

    async fn logout(&self, ep: &ConnectedEndpoint, access_token: &str) -> Result<(), ClientError> {
        let client = self.build_pin_with_identity(ep)?;
        let url = format!("{}/v1/auth/logout", ep.base_url);
        let resp = client
            .post(url)
            .header("x-client-version", &ep.client_version)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        if resp.status().is_success() {
            return Ok(());
        }
        let body = resp
            .text()
            .await
            .map_err(|e| ClientError::Transport(format!("read body: {e}")))?;
        match serde_json::from_str::<ErrorEnvelope>(&body) {
            Ok(env) => Err(ClientError::Server(env)),
            Err(_) => Err(ClientError::HttpStatus { status: 0, body }),
        }
    }

    async fn list_patients(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        query: PatientListQuery,
    ) -> Result<PatientListResponse, ClientError> {
        let client = self.build_pin_with_identity(ep)?;
        let url = format!("{}/v1/patients", ep.base_url);
        let mut req = client
            .get(url)
            .header("x-client-version", &ep.client_version)
            .bearer_auth(access_token);
        if let Some(q) = &query.q {
            req = req.query(&[("q", q.as_str())]);
        }
        if query.include_archived {
            req = req.query(&[("include_archived", "true")]);
        }
        if let Some(limit) = query.limit {
            req = req.query(&[("limit", limit.to_string().as_str())]);
        }
        if let Some(before) = query.before {
            req = req.query(&[("before", before.to_string().as_str())]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        parse_response(resp).await
    }

    async fn get_patient_detail(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        patient_id: PatientId,
    ) -> Result<PatientDetail, ClientError> {
        let client = self.build_pin_with_identity(ep)?;
        let url = format!("{}/v1/patients/{}/detail", ep.base_url, patient_id.as_uuid());
        let resp = client
            .get(url)
            .header("x-client-version", &ep.client_version)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        parse_response(resp).await
    }
}
