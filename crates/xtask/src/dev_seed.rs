//! `cargo xtask dev-seed` — talk HTTP to a running `anamnez serve` (must have been
//! launched by `dev-up`), mint an enrollment URI for the Tauri workstation, and
//! seed a `[TEST]` patient + an in-progress encounter.
//!
//! Reads the dev workstation cert from `data_dir/dev-workstation/` (laid down by
//! dev-up's init phase) and uses it as the mTLS identity.

use jiff::Timestamp;
use reqwest::header::HeaderValue;
use reqwest::tls::{Certificate, Identity};
use reqwest::{Client, ClientBuilder, StatusCode};
use serde_json::{json, Value};

use crate::paths;

pub async fn run(_args: Vec<String>) -> Result<(), String> {
    let base = format!("https://{}", paths::BIND);

    let ca_pem = std::fs::read_to_string(paths::dev_workstation_dir().join("ca.pem"))
        .map_err(|e| format!("read ca.pem: {e} — run `cargo xtask dev-up` first"))?;
    let cert_pem = std::fs::read_to_string(paths::dev_workstation_cert())
        .map_err(|e| format!("read dev workstation cert: {e}"))?;
    let key_pem = std::fs::read_to_string(paths::dev_workstation_key())
        .map_err(|e| format!("read dev workstation key: {e}"))?;

    let ca = Certificate::from_pem(ca_pem.as_bytes()).map_err(|e| format!("ca parse: {e}"))?;
    let identity_pem = format!("{cert_pem}\n{key_pem}");
    let identity = Identity::from_pem(identity_pem.as_bytes())
        .map_err(|e| format!("identity parse: {e}"))?;
    let client: Client = ClientBuilder::new()
        .add_root_certificate(ca)
        .identity(identity)
        .danger_accept_invalid_hostnames(true)
        .build()
        .map_err(|e| format!("reqwest build: {e}"))?;

    let v = HeaderValue::from_static("1.0.0");

    // 1. Login as the dev admin.
    let login_body = json!({
        "email": paths::ADMIN_EMAIL,
        "password": paths::ADMIN_PASSWORD,
        "client_version": "1.0.0",
    });
    let r = client
        .post(format!("{base}/v1/auth/login"))
        .header("x-client-version", &v)
        .json(&login_body)
        .send()
        .await
        .map_err(|e| format!("login transport: {e} — is the daemon running?"))?;
    if r.status() != StatusCode::OK {
        let body = r.text().await.unwrap_or_default();
        return Err(format!("login failed: {body}"));
    }
    let login: Value = r.json().await.map_err(|e| format!("login json: {e}"))?;
    let access = login["access_token"]
        .as_str()
        .ok_or("login response missing access_token")?
        .to_owned();
    eprintln!(
        "dev-seed: logged in as {} (environment={}, idle_lock_minutes={})",
        login["user"]["email"].as_str().unwrap_or("?"),
        login["environment"].as_str().unwrap_or("?"),
        login["idle_lock_minutes"]
    );

    // 2. Mint an enrollment URI for the Tauri workstation.
    let r = client
        .post(format!("{base}/v1/admin/workstations"))
        .header("x-client-version", &v)
        .bearer_auth(&access)
        .header("x-step-up-password", paths::ADMIN_PASSWORD)
        .json(&json!({
            "label": "Tauri Workstation (dev)",
            "mode": "shared",
            "host": "127.0.0.1:8443",
        }))
        .send()
        .await
        .map_err(|e| format!("mint enrollment transport: {e}"))?;
    if r.status() != StatusCode::OK {
        let body = r.text().await.unwrap_or_default();
        return Err(format!("mint enrollment failed: {body}"));
    }
    let minted: Value = r.json().await.map_err(|e| format!("mint json: {e}"))?;
    let uri = minted["uri"]
        .as_str()
        .ok_or("mint response missing uri")?
        .to_owned();
    std::fs::write(paths::last_enrollment_uri_file(), &uri)
        .map_err(|e| format!("persist uri: {e}"))?;

    // 3. Seed a `[TEST]` patient (unique per run via timestamp suffix).
    let ts = Timestamp::now().to_string();
    let suffix = &ts[..ts.len().min(19)]; // YYYY-MM-DDTHH:MM:SS
    let r = client
        .post(format!("{base}/v1/patients"))
        .header("x-client-version", &v)
        .bearer_auth(&access)
        .json(&json!({
            "mrn": format!("DEV-{suffix}"),
            "given_names": format!("[TEST] Hasta {suffix}"),
            "family_name": "[TEST] Soyad",
            "preferred_name": null,
            "date_of_birth": "1980-01-01",
            "sex_assigned_at_birth": "unknown",
            "gender_identity": null,
            "email": null,
            "phone": null,
            "address": null,
            "emergency_contact_name": null,
            "emergency_contact_phone": null,
            "emergency_contact_relationship": null,
        }))
        .send()
        .await
        .map_err(|e| format!("create patient transport: {e}"))?;
    if r.status() != StatusCode::OK {
        let body = r.text().await.unwrap_or_default();
        return Err(format!("create patient failed: {body}"));
    }
    let patient: Value = r.json().await.map_err(|e| format!("patient json: {e}"))?;
    let patient_id = patient["value"]["id"]
        .as_str()
        .ok_or("patient response missing id")?
        .to_owned();

    // 4. Open an encounter (no code needed while in_progress).
    let r = client
        .post(format!("{base}/v1/encounters"))
        .header("x-client-version", &v)
        .bearer_auth(&access)
        .json(&json!({
            "patient_id": patient_id,
            "kind": "in_person",
            "reason_text": "Genel kontrol",
        }))
        .send()
        .await
        .map_err(|e| format!("start encounter transport: {e}"))?;
    if r.status() != StatusCode::OK {
        let body = r.text().await.unwrap_or_default();
        return Err(format!("start encounter failed: {body}"));
    }

    // 5. Add a free-text allergy (no code required).
    let r = client
        .post(format!("{base}/v1/allergies"))
        .header("x-client-version", &v)
        .bearer_auth(&access)
        .json(&json!({
            "patient_id": patient_id,
            "code": null,
            "code_system": null,
            "display_text": "fıstık",
            "severity": "moderate",
            "reaction_text": "kaşıntı",
            "status": "active",
            "onset_date": null,
            "source_id": null,
            "encounter_id": null,
        }))
        .send()
        .await
        .map_err(|e| format!("create allergy transport: {e}"))?;
    if r.status() != StatusCode::OK {
        let body = r.text().await.unwrap_or_default();
        return Err(format!("create allergy failed: {body}"));
    }

    eprintln!();
    eprintln!("dev-seed: seeded patient_id={patient_id}");
    eprintln!();
    eprintln!("Paste this into the Tauri workstation's enrollment screen:");
    eprintln!();
    eprintln!("  {uri}");
    eprintln!();
    eprintln!("Then log in as:");
    eprintln!("  email:    {}", paths::ADMIN_EMAIL);
    eprintln!("  password: {}", paths::ADMIN_PASSWORD);
    eprintln!();
    Ok(())
}
