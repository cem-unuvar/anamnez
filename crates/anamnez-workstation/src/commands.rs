//! Tauri IPC commands. The Leptos WASM in the webview calls these via `invoke`.
//!
//! The "transport" commands forward to `NativeTransport`. The lifecycle commands
//! (enroll, login, logout, seal_session, bootstrap_state) own the workstation's
//! session and config state — the WASM is purely a view, with no secrets.
//!
//! Per SPEC §Workstation client → Stack: "TLS to the server goes through `rustls`
//! on the Tauri side, not through the webview's stack."

use anamnez_client_core::error::ClientError;
use anamnez_client_core::secret_native::{self, Slot};
use anamnez_client_core::transport::{ConnectedEndpoint, EnrollEndpoint, HttpTransport};
use anamnez_client_core::Session;
use anamnez_protocol::auth::{LoginRequest, LoginResponse, RefreshRequest, RefreshResponse};
use anamnez_protocol::enroll::{EnrollExchangeRequest, EnrollExchangeResponse};
use anamnez_protocol::environment::Environment;
use anamnez_protocol::health::HealthEnvelope;
use anamnez_protocol::ids::PatientId;
use anamnez_protocol::patient::{PatientDetail, PatientListQuery, PatientListResponse};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::config::{self, Config, DaemonConfig};
use crate::state::AppState;

/// IPC envelope used by `transport_*` commands. The WASM-side `transport_tauri`
/// deserializes this discriminant union and maps it to `ClientError`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcReply<T> {
    Ok(T),
    Err(anamnez_protocol::error::ErrorEnvelope),
    Transport(String),
}

fn wrap<T>(r: Result<T, ClientError>) -> IpcReply<T> {
    match r {
        Ok(v) => IpcReply::Ok(v),
        Err(ClientError::Server(env)) => IpcReply::Err(env),
        Err(ClientError::Transport(s)) | Err(ClientError::HttpStatus { body: s, .. }) => {
            IpcReply::Transport(s)
        }
        Err(ClientError::Serde(s)) => IpcReply::Transport(format!("serde: {s}")),
        Err(ClientError::Ipc(s)) => IpcReply::Transport(format!("ipc: {s}")),
    }
}

// ─── Lifecycle / bootstrap commands ───────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BootstrapState {
    pub has_workstation_credential: bool,
    pub has_refresh_token: bool,
    pub idle_lock_minutes_cache: u32,
    pub daemon: Option<DaemonConfig>,
    /// Path the config file is read from / written to — surfaced to the UI for
    /// diagnostics in error states.
    pub config_path: String,
}

#[tauri::command]
pub async fn bootstrap_state() -> Result<BootstrapState, String> {
    let cfg = config::load().map_err(|e| e.to_string())?;
    let path = config::config_path()
        .map_err(|e| e.to_string())?
        .display()
        .to_string();
    let cert = secret_native::get(Slot::DeviceCertPem)
        .map_err(|e| e.to_string())?
        .is_some();
    let refresh = secret_native::get(Slot::RefreshToken)
        .map_err(|e| e.to_string())?
        .is_some();
    Ok(BootstrapState {
        has_workstation_credential: cert && cfg.daemon.is_some(),
        has_refresh_token: refresh,
        idle_lock_minutes_cache: cfg.idle_lock_minutes_cache,
        daemon: cfg.daemon,
        config_path: path,
    })
}

#[derive(Debug, Serialize)]
pub struct EnrollOutcome {
    pub workstation_id: String,
    pub daemon: DaemonConfig,
}

#[tauri::command]
pub async fn enroll_from_uri(
    state: State<'_, AppState>,
    uri: String,
) -> Result<EnrollOutcome, String> {
    let parsed = parse_enroll_uri(&uri).map_err(|e| e.to_string())?;
    let enroll_ep = EnrollEndpoint {
        base_url: format!("https://{}", parsed.host),
        server_fingerprint_sha256: parsed.fingerprint.clone(),
        client_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    let resp = state
        .transport
        .enroll_exchange(
            &enroll_ep,
            EnrollExchangeRequest {
                token: parsed.token,
                client_version: env!("CARGO_PKG_VERSION").to_owned(),
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    // Persist the device credential in the OS secret store.
    secret_native::put(Slot::DeviceCertPem, &resp.client_cert_pem).map_err(|e| e.to_string())?;
    secret_native::put(Slot::DeviceKeyPem, &resp.client_key_pem).map_err(|e| e.to_string())?;
    secret_native::put(Slot::CaCertPem, &resp.ca_cert_pem).map_err(|e| e.to_string())?;

    // Persist the daemon config alongside.
    let daemon = DaemonConfig {
        base_url: enroll_ep.base_url.clone(),
        server_fingerprint_sha256: parsed.fingerprint,
        workstation_id: resp.workstation_id.as_uuid().to_string(),
    };
    let mut cfg = config::load().map_err(|e| e.to_string())?;
    cfg.daemon = Some(daemon.clone());
    config::save(&cfg).map_err(|e| e.to_string())?;

    // Set the connected endpoint in memory so login + subsequent calls can use it.
    *state.connected.write() = Some(ConnectedEndpoint {
        base_url: daemon.base_url.clone(),
        server_fingerprint_sha256: daemon.server_fingerprint_sha256.clone(),
        ca_cert_pem: resp.ca_cert_pem,
        client_cert_pem: resp.client_cert_pem,
        client_key_pem: resp.client_key_pem,
        client_version: env!("CARGO_PKG_VERSION").to_owned(),
    });

    Ok(EnrollOutcome {
        workstation_id: daemon.workstation_id.clone(),
        daemon,
    })
}

#[derive(Debug, Serialize)]
pub struct LoginEcho {
    pub user: anamnez_protocol::auth::User,
    pub environment: Environment,
    pub idle_lock_minutes: u32,
}

#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<LoginEcho, String> {
    // Ensure the connected endpoint is loaded — boot path may have skipped this if
    // the user just enrolled and immediately tries to log in.
    ensure_connected(&state).await.map_err(|e| e.to_string())?;
    let ep = state
        .connected
        .read()
        .clone()
        .ok_or_else(|| "no daemon configured — finish enrollment first".to_string())?;
    let resp = state
        .transport
        .login(
            &ep,
            LoginRequest {
                email,
                password,
                client_version: env!("CARGO_PKG_VERSION").to_owned(),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    let session = Session::from_login(resp);

    secret_native::put(Slot::RefreshToken, &session.refresh_token)
        .map_err(|e| e.to_string())?;
    *state.access_token.write() = Some(session.access_token.clone());

    // Cache the idle-lock policy returned by the server.
    let mut cfg = config::load().map_err(|e| e.to_string())?;
    cfg.idle_lock_minutes_cache = session.idle_lock_minutes;
    config::save(&cfg).map_err(|e| e.to_string())?;

    Ok(LoginEcho {
        user: session.user,
        environment: session.environment,
        idle_lock_minutes: session.idle_lock_minutes,
    })
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    let ep = state.connected.read().clone();
    let access = state.access_token.read().clone();
    if let (Some(ep), Some(access)) = (ep, access) {
        // Best-effort; even if the server rejects, scrub local state.
        let _ = state.transport.logout(&ep, &access).await;
    }
    secret_native::delete(Slot::RefreshToken).map_err(|e| e.to_string())?;
    *state.access_token.write() = None;
    Ok(())
}

/// Drop the in-memory access token. Subsequent authed `transport_*` calls fail
/// with `SessionExpired` until the user logs in again. Triggered by the WASM
/// idle-lock guard.
#[tauri::command]
pub async fn seal_session(state: State<'_, AppState>) -> Result<(), String> {
    *state.access_token.write() = None;
    Ok(())
}

#[tauri::command]
pub async fn current_environment(state: State<'_, AppState>) -> Result<Option<Environment>, String> {
    // Best-effort: ask /v1/health if we have a connected endpoint.
    let ep = state.connected.read().clone();
    let Some(ep) = ep else { return Ok(None) };
    match state.transport.health(&ep).await {
        Ok(h) => Ok(Some(h.environment)),
        Err(_) => Ok(None),
    }
}

// ─── Transport pass-through commands (called by `transport_tauri`) ────────────

#[tauri::command]
pub async fn transport_health(
    state: State<'_, AppState>,
    ep: ConnectedEndpoint,
) -> Result<IpcReply<HealthEnvelope>, String> {
    Ok(wrap(state.transport.health(&ep).await))
}

#[tauri::command]
pub async fn transport_enroll_exchange(
    state: State<'_, AppState>,
    ep: EnrollEndpoint,
    req: EnrollExchangeRequest,
) -> Result<IpcReply<EnrollExchangeResponse>, String> {
    Ok(wrap(state.transport.enroll_exchange(&ep, req).await))
}

#[tauri::command]
pub async fn transport_login(
    state: State<'_, AppState>,
    ep: ConnectedEndpoint,
    req: LoginRequest,
) -> Result<IpcReply<LoginResponse>, String> {
    Ok(wrap(state.transport.login(&ep, req).await))
}

#[tauri::command]
pub async fn transport_refresh(
    state: State<'_, AppState>,
    ep: ConnectedEndpoint,
    req: RefreshRequest,
) -> Result<IpcReply<RefreshResponse>, String> {
    Ok(wrap(state.transport.refresh(&ep, req).await))
}

#[tauri::command]
pub async fn transport_logout(
    state: State<'_, AppState>,
    ep: ConnectedEndpoint,
    access_token: String,
) -> Result<IpcReply<()>, String> {
    Ok(wrap(state.transport.logout(&ep, &access_token).await))
}

#[tauri::command]
pub async fn transport_list_patients(
    state: State<'_, AppState>,
    ep: ConnectedEndpoint,
    access_token: String,
    query: PatientListQuery,
) -> Result<IpcReply<PatientListResponse>, String> {
    Ok(wrap(
        state
            .transport
            .list_patients(&ep, &access_token, query)
            .await,
    ))
}

#[tauri::command]
pub async fn transport_get_patient_detail(
    state: State<'_, AppState>,
    ep: ConnectedEndpoint,
    access_token: String,
    patient_id: PatientId,
) -> Result<IpcReply<PatientDetail>, String> {
    Ok(wrap(
        state
            .transport
            .get_patient_detail(&ep, &access_token, patient_id)
            .await,
    ))
}

// ─── UI-friendly commands that read native state ──────────────────────────────
// The WASM UI shouldn't see cert PEMs or the access token — those live in the OS
// keychain and in-process state. These commands take ONLY the request data; the
// native side fills in the rest.

#[tauri::command]
pub async fn ui_list_patients(
    state: State<'_, AppState>,
    query: PatientListQuery,
) -> Result<IpcReply<PatientListResponse>, String> {
    let ep = state.connected.read().clone();
    let token = state.access_token.read().clone();
    let (Some(ep), Some(token)) = (ep, token) else {
        return Ok(IpcReply::Err(anamnez_protocol::error::ErrorEnvelope::SessionExpired));
    };
    Ok(wrap(state.transport.list_patients(&ep, &token, query).await))
}

#[tauri::command]
pub async fn ui_get_patient_detail(
    state: State<'_, AppState>,
    patient_id: PatientId,
) -> Result<IpcReply<PatientDetail>, String> {
    let ep = state.connected.read().clone();
    let token = state.access_token.read().clone();
    let (Some(ep), Some(token)) = (ep, token) else {
        return Ok(IpcReply::Err(anamnez_protocol::error::ErrorEnvelope::SessionExpired));
    };
    Ok(wrap(
        state
            .transport
            .get_patient_detail(&ep, &token, patient_id)
            .await,
    ))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn ensure_connected(state: &State<'_, AppState>) -> Result<(), String> {
    if state.connected.read().is_some() {
        return Ok(());
    }
    let cfg: Config = config::load().map_err(|e| e.to_string())?;
    let Some(daemon) = cfg.daemon else {
        return Err("workstation not enrolled".into());
    };
    let cert = secret_native::get(Slot::DeviceCertPem)
        .map_err(|e| e.to_string())?
        .ok_or("device cert missing from secret store")?;
    let key = secret_native::get(Slot::DeviceKeyPem)
        .map_err(|e| e.to_string())?
        .ok_or("device key missing from secret store")?;
    let ca = secret_native::get(Slot::CaCertPem)
        .map_err(|e| e.to_string())?
        .ok_or("ca cert missing from secret store")?;
    *state.connected.write() = Some(ConnectedEndpoint {
        base_url: daemon.base_url,
        server_fingerprint_sha256: daemon.server_fingerprint_sha256,
        ca_cert_pem: ca,
        client_cert_pem: cert,
        client_key_pem: key,
        client_version: env!("CARGO_PKG_VERSION").to_owned(),
    });
    Ok(())
}

#[derive(Debug)]
struct ParsedEnrollUri {
    host: String,
    fingerprint: String,
    token: String,
}

fn parse_enroll_uri(uri: &str) -> Result<ParsedEnrollUri, String> {
    let rest = uri
        .strip_prefix("anamnez://enroll?")
        .ok_or("not an anamnez://enroll URI")?;
    let mut host = None;
    let mut fingerprint = None;
    let mut token = None;
    for pair in rest.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        let v = url_decode(v);
        match k {
            "host" => host = Some(v),
            "fingerprint" => fingerprint = Some(v),
            "token" => token = Some(v),
            _ => {}
        }
    }
    Ok(ParsedEnrollUri {
        host: host.ok_or("uri missing host")?,
        fingerprint: fingerprint.ok_or("uri missing fingerprint")?,
        token: token.ok_or("uri missing token")?,
    })
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_enroll_uri() {
        let parsed = parse_enroll_uri(
            "anamnez://enroll?host=10.0.0.5%3A8443&fingerprint=AB%3ACD&token=abc123",
        )
        .unwrap();
        assert_eq!(parsed.host, "10.0.0.5:8443");
        assert_eq!(parsed.fingerprint, "AB:CD");
        assert_eq!(parsed.token, "abc123");
    }

    #[test]
    fn rejects_other_schemes() {
        assert!(parse_enroll_uri("https://example/").is_err());
    }
}
