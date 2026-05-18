//! WASM-side transport: forwards every `HttpTransport` method to a Tauri command via
//! `__TAURI_INVOKE__`. The Tauri shell registers matching commands and executes the
//! actual HTTP through `transport_native::NativeTransport`. See SPEC §Workstation
//! client → Stack: "TLS to the server goes through `rustls` on the Tauri side, not
//! through the webview's stack."

use anamnez_protocol::auth::{LoginRequest, LoginResponse, RefreshRequest, RefreshResponse};
use anamnez_protocol::enroll::{EnrollExchangeRequest, EnrollExchangeResponse};
use anamnez_protocol::error::ErrorEnvelope;
use anamnez_protocol::health::HealthEnvelope;
use anamnez_protocol::ids::PatientId;
use anamnez_protocol::patient::{PatientDetail, PatientListQuery, PatientListResponse};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::error::ClientError;
use crate::transport::{ConnectedEndpoint, EnrollEndpoint, HttpTransport};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI_INTERNALS__"], js_name = invoke)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> js_sys::Promise;
}

#[derive(Debug, Default)]
pub struct TauriTransport;

impl TauriTransport {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// IPC envelope: native side returns either `{ ok: T }` or `{ err: ErrorEnvelope }`.
/// Using an explicit wrapper avoids depending on the Tauri command error-handling
/// behavior (which differs between throwing JS errors and returning Result).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum IpcReply<T> {
    Ok(T),
    Err(ErrorEnvelope),
    /// Transport-level failure on the native side (`reqwest::Error` etc.).
    /// Mapped to `ClientError::Transport`.
    Transport(String),
}

async fn invoke<R: serde::de::DeserializeOwned>(
    cmd: &str,
    args: impl serde::Serialize,
) -> Result<R, ClientError> {
    let args_js = serde_wasm_bindgen::to_value(&args)
        .map_err(|e| ClientError::Ipc(format!("serialize args: {e}")))?;
    let promise = tauri_invoke(cmd, args_js);
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| ClientError::Ipc(format!("invoke: {e:?}")))?;
    let reply: IpcReply<R> = serde_wasm_bindgen::from_value(result)
        .map_err(|e| ClientError::Ipc(format!("deserialize reply: {e}")))?;
    match reply {
        IpcReply::Ok(v) => Ok(v),
        IpcReply::Err(env) => Err(ClientError::Server(env)),
        IpcReply::Transport(s) => Err(ClientError::Transport(s)),
    }
}

#[derive(Serialize)]
struct HealthArgs<'a> {
    ep: &'a ConnectedEndpoint,
}
#[derive(Serialize)]
struct ExchangeArgs<'a> {
    ep: &'a EnrollEndpoint,
    req: &'a EnrollExchangeRequest,
}
#[derive(Serialize)]
struct LoginArgs<'a> {
    ep: &'a ConnectedEndpoint,
    req: &'a LoginRequest,
}
#[derive(Serialize)]
struct RefreshArgs<'a> {
    ep: &'a ConnectedEndpoint,
    req: &'a RefreshRequest,
}
#[derive(Serialize)]
struct LogoutArgs<'a> {
    ep: &'a ConnectedEndpoint,
    access_token: &'a str,
}
#[derive(Serialize)]
struct ListPatientsArgs<'a> {
    ep: &'a ConnectedEndpoint,
    access_token: &'a str,
    query: &'a PatientListQuery,
}
#[derive(Serialize)]
struct GetPatientDetailArgs<'a> {
    ep: &'a ConnectedEndpoint,
    access_token: &'a str,
    patient_id: PatientId,
}

#[async_trait(?Send)]
impl HttpTransport for TauriTransport {
    async fn health(&self, ep: &ConnectedEndpoint) -> Result<HealthEnvelope, ClientError> {
        invoke("transport_health", HealthArgs { ep }).await
    }

    async fn enroll_exchange(
        &self,
        ep: &EnrollEndpoint,
        req: EnrollExchangeRequest,
    ) -> Result<EnrollExchangeResponse, ClientError> {
        invoke("transport_enroll_exchange", ExchangeArgs { ep, req: &req }).await
    }

    async fn login(
        &self,
        ep: &ConnectedEndpoint,
        req: LoginRequest,
    ) -> Result<LoginResponse, ClientError> {
        invoke("transport_login", LoginArgs { ep, req: &req }).await
    }

    async fn refresh(
        &self,
        ep: &ConnectedEndpoint,
        req: RefreshRequest,
    ) -> Result<RefreshResponse, ClientError> {
        invoke("transport_refresh", RefreshArgs { ep, req: &req }).await
    }

    async fn logout(&self, ep: &ConnectedEndpoint, access_token: &str) -> Result<(), ClientError> {
        invoke("transport_logout", LogoutArgs { ep, access_token }).await
    }

    async fn list_patients(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        query: PatientListQuery,
    ) -> Result<PatientListResponse, ClientError> {
        invoke(
            "transport_list_patients",
            ListPatientsArgs {
                ep,
                access_token,
                query: &query,
            },
        )
        .await
    }

    async fn get_patient_detail(
        &self,
        ep: &ConnectedEndpoint,
        access_token: &str,
        patient_id: PatientId,
    ) -> Result<PatientDetail, ClientError> {
        invoke(
            "transport_get_patient_detail",
            GetPatientDetailArgs {
                ep,
                access_token,
                patient_id,
            },
        )
        .await
    }
}
