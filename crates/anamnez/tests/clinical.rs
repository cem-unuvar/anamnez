//! Layer-2: clinical write round-trip. Full pipeline of TLS → auth → route handler
//! → core function → audit + DB → wire DTO round-trip.

mod support;

use std::time::Duration;
use support::{api::Api, bootstrap, spawn, tls};

#[tokio::test(flavor = "multi_thread")]
async fn observation_create_round_trip() {
    let bs = bootstrap::fresh();
    let port = spawn::pick_free_port();
    let bind = format!("127.0.0.1:{port}");
    let base = format!("https://127.0.0.1:{port}");
    let config_path = bs.data_dir.join("config.toml");
    let pid_path = bs.data_dir.join("anamnez.pid");
    let code_systems = bs.data_dir.join("code-systems");
    std::fs::create_dir_all(&code_systems).unwrap();
    std::fs::create_dir_all(bs.data_dir.join("blobs")).unwrap();
    std::fs::write(
        &config_path,
        bootstrap::config_toml(&bs.data_dir, &code_systems),
    )
    .unwrap();

    // Codes are required on every observation; seed one ANAMNEZ-SYM row so the
    // observation create below has a valid `(code, code_system)` pair to point at.
    bootstrap::seed_symptom(&bs.data_dir, "ANAMNEZ-SYM-0042", "boyun ağrısı");

    let _daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);
    let client = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    let ready = spawn::wait_for_ready(&client, &base, Duration::from_secs(15)).await;
    assert!(ready, "daemon not ready");

    let mut api = Api::new(client, base.clone());
    let body = serde_json::json!({
        "email": bs.admin_email,
        "password": bs.admin_password,
        "client_version": "1.0.0",
    });
    let resp = api.post_raw("/v1/auth/login", &body).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    api.bearer = Some(json["access_token"].as_str().unwrap().to_owned());

    // Admin creates a patient — auto-owner gives admin write_clinical access.
    let new_patient = serde_json::json!({
        "mrn": null,
        "given_names": "[TEST] Maria",
        "family_name": "[TEST] Doe",
        "preferred_name": null,
        "date_of_birth": "1990-04-15",
        "sex_assigned_at_birth": "female",
        "gender_identity": null,
        "email": null,
        "phone": null,
        "address": null,
        "emergency_contact_name": null,
        "emergency_contact_phone": null,
        "emergency_contact_relationship": null,
    });
    let resp = api.post_raw("/v1/patients", &new_patient).await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "patient create: {}",
        resp.text().await.unwrap_or_default()
    );
    let patient_v: serde_json::Value = resp.json().await.unwrap();
    let patient_id = patient_v["value"]["id"].as_str().unwrap().to_owned();
    assert_eq!(patient_v["version"], 1);

    // Create an observation.
    let new_obs = serde_json::json!({
        "patient_id": patient_id,
        "effective_period_start": "2026-05-17T10:00:00Z",
        "effective_period_end": null,
        "code": "ANAMNEZ-SYM-0042",
        "code_system": "ANAMNEZ-SYM",
        "display_text": "boyun ağrısı",
        "value": null,
        "status": "preliminary",
        "is_problem_list_item": false,
        "source_id": null,
        "encounter_id": null,
        "extracted_by": "manual",
        "model_version": null,
        "confidence": null,
    });
    let resp = api.post_raw("/v1/observations", &new_obs).await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "obs create: {}",
        resp.text().await.unwrap_or_default()
    );
    let obs_v: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(obs_v["version"], 1);
    assert_eq!(obs_v["value"]["display_text"], "boyun ağrısı");
}
