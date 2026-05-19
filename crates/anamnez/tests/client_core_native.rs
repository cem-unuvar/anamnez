//! Layer 2: exercise the `anamnez-client-core` native transport end-to-end against
//! a spawned daemon. Covers the workstation client's "happy path":
//!
//!     health → enroll_exchange → login → list_patients → get_patient_detail
//!
//! The native transport is what the Tauri shell uses in production. The wasm transport
//! is exercised at runtime in the webview only — not covered by `cargo nextest`.

mod support;

use std::time::Duration;
use support::{api::Api, bootstrap, spawn, tls};

use anamnez_client_core::transport::{ConnectedEndpoint, EnrollEndpoint};
use anamnez_client_core::transport_native::NativeTransport;
use anamnez_client_core::HttpTransport;
use anamnez_protocol::auth::LoginRequest;
use anamnez_protocol::codesystem::CodeSystem;
use anamnez_protocol::encounter::EncounterStatus;
use anamnez_protocol::enroll::EnrollExchangeRequest;
use anamnez_protocol::environment::Environment;
use anamnez_protocol::ids::PatientId;
use anamnez_protocol::observation::{ObservationStatus, ObservationValue};
use anamnez_protocol::patient::PatientListQuery;
use sha2::{Digest, Sha256};

fn fingerprint_sha256_hex_of_pem_leaf(pem: &str) -> String {
    let mut cursor = std::io::Cursor::new(pem.as_bytes());
    let der = rustls_pemfile::certs(&mut cursor)
        .next()
        .expect("server cert pem: empty")
        .expect("server cert pem: invalid");
    let mut h = Sha256::new();
    h.update(der.as_ref());
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[tokio::test(flavor = "multi_thread")]
async fn native_transport_walks_enroll_login_list_detail() {
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
    // Codes are required on every observation; seed a single ANAMNEZ-SYM row
    // so the lab-style POST below has a valid `(code_system, code)` pair.
    bootstrap::seed_symptom(&bs.data_dir, "ANAMNEZ-SYM-LAB", "LDL Kolesterol");
    let _daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);

    // Wait for daemon to be ready using the warm (mTLS-equipped) reqwest client.
    let warm = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    assert!(spawn::wait_for_ready(&warm, &base, Duration::from_secs(15)).await);

    let fingerprint = fingerprint_sha256_hex_of_pem_leaf(&bs.server_cert_pem);
    let enroll_ep = EnrollEndpoint {
        base_url: base.clone(),
        server_fingerprint_sha256: fingerprint.clone(),
        client_version: "1.0.0".into(),
    };
    let transport = NativeTransport::new();

    // /v1/health requires mTLS (require_device_id middleware) — only /v1/enroll/exchange
    // is reachable without a client cert. Pre-enrollment, the bootstrap UI shows no
    // TEST shield; the shield appears between enrollment and login, sourced from
    // a post-enrollment /v1/health call (below).

    // 1. Mint enrollment via admin HTTP route (existing pattern from enroll.rs).
    let mut admin = Api::new(warm.clone(), base.clone());
    let r = admin
        .post_raw(
            "/v1/auth/login",
            &serde_json::json!({
                "email": bs.admin_email,
                "password": bs.admin_password,
                "client_version": "1.0.0",
            }),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    let login: serde_json::Value = r.json().await.unwrap();
    admin.bearer = Some(login["access_token"].as_str().unwrap().into());
    admin.stepup_password = Some(bs.admin_password.clone());

    let r = admin
        .post_raw(
            "/v1/admin/workstations",
            &serde_json::json!({
                "label": "Native Transport Test",
                "mode": "shared",
                "host": "127.0.0.1",
            }),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    let minted: serde_json::Value = r.json().await.unwrap();
    let token = minted["token"].as_str().unwrap().to_string();

    // 2. enroll_exchange through the native transport. No client identity.
    let exchanged = transport
        .enroll_exchange(
            &enroll_ep,
            EnrollExchangeRequest {
                token,
                client_version: "1.0.0".into(),
            },
        )
        .await
        .expect("enroll_exchange");

    // 3. Build a ConnectedEndpoint from the returned identity and log in.
    let connected = ConnectedEndpoint {
        base_url: base.clone(),
        server_fingerprint_sha256: fingerprint.clone(),
        ca_cert_pem: exchanged.ca_cert_pem,
        client_cert_pem: exchanged.client_cert_pem,
        client_key_pem: exchanged.client_key_pem,
        client_version: "1.0.0".into(),
    };

    // 4. health (post-enrollment, mTLS) returns the daemon's environment.
    let h = transport
        .health(&connected)
        .await
        .expect("health post-enroll");
    assert_eq!(h.environment, Environment::Test);

    // 5. login carries environment + idle_lock_minutes.
    let login = transport
        .login(
            &connected,
            LoginRequest {
                email: bs.admin_email.clone(),
                password: bs.admin_password.clone(),
                client_version: "1.0.0".into(),
            },
        )
        .await
        .expect("login");
    assert_eq!(login.environment, Environment::Test);
    assert!(login.idle_lock_minutes >= 5 && login.idle_lock_minutes <= 30);
    let access = login.access_token.clone();

    // 6. list_patients on a fresh DB returns empty.
    let list = transport
        .list_patients(&connected, &access, PatientListQuery::default())
        .await
        .expect("list_patients");
    assert!(list.items.is_empty());
    assert!(list.next_before.is_none());

    // 7. Seed a patient + open encounter via the admin HTTP route, then fetch
    //    /v1/patients/:id/detail through the NATIVE TRANSPORT. This is the
    //    exact path the Tauri workstation takes when a clinician opens a
    //    patient page, and it deserializes the PatientDetail JSON through
    //    reqwest's `.json::<PatientDetail>()`. Regression for the bug where a
    //    protocol shape change (encounters: Vec<Encounter> →
    //    Vec<Versioned<Encounter>>) shipped without a matching daemon rebuild,
    //    causing the workstation to fail every patient page with the opaque
    //    "serde: error decoding response body" message.
    let r = admin
        .post_raw(
            "/v1/patients",
            &serde_json::json!({
                "mrn": "REGRESSION-001",
                "given_names": "[TEST] Regression",
                "family_name": "[TEST] Test",
                "preferred_name": null,
                "date_of_birth": "1990-01-01",
                "sex_assigned_at_birth": "unknown",
                "gender_identity": null,
                "email": null,
                "phone": null,
                "address": null,
                "emergency_contact_name": null,
                "emergency_contact_phone": null,
                "emergency_contact_relationship": null,
            }),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    let created: serde_json::Value = r.json().await.unwrap();
    let patient_id_str = created["value"]["id"].as_str().unwrap().to_owned();
    let patient_id = PatientId(uuid::Uuid::parse_str(&patient_id_str).unwrap());

    let r = admin
        .post_raw(
            "/v1/encounters",
            &serde_json::json!({
                "patient_id": patient_id_str,
                "kind": "in_person",
                "reason_text": "regression encounter",
            }),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), reqwest::StatusCode::OK);

    // The actual regression assertion: the native transport must be able to
    // parse the daemon's PatientDetail response, with the encounter coming
    // back inside a `Versioned` wrapper that exposes `expected_version`.
    let detail = transport
        .get_patient_detail(&connected, &access, patient_id)
        .await
        .expect("get_patient_detail with seeded encounter");
    assert_eq!(detail.patient.given_names, "[TEST] Regression");
    assert_eq!(
        detail.encounters.len(),
        1,
        "expected the seeded encounter in detail.encounters",
    );
    let enc = &detail.encounters[0];
    assert!(enc.version >= 1, "encounter version should round-trip");
    assert!(
        matches!(enc.value.status, EncounterStatus::InProgress),
        "expected the seeded encounter to be InProgress, got {:?}",
        enc.value.status,
    );
    assert_eq!(enc.value.reason_text, "regression encounter");

    // 8. Lab-style observation: post a free-text observation carrying a
    //    `ValueQuantity` (the shape the workstation form sends when the
    //    clinician types "LDL Kolesterol" + "130" + "mg/dL"). Regression for
    //    the bug where `ui_create_observation` hardcoded `value: None`,
    //    making it impossible to record lab results from the UI even though
    //    the wire protocol supported them.
    //
    //    Codes are required on every observation; we seeded an ANAMNEZ-SYM
    //    row above so this round-trip has a valid `(code, code_system)` pair
    //    to point at without loading the full code-systems bundle.
    let r = admin
        .post_raw(
            "/v1/observations",
            &serde_json::json!({
                "patient_id": patient_id_str,
                "effective_period_start": jiff::Timestamp::now().to_string(),
                "effective_period_end": null,
                "code": "ANAMNEZ-SYM-LAB",
                "code_system": "ANAMNEZ-SYM",
                "display_text": "[TEST] LDL Kolesterol",
                "value": { "value": 130.0, "unit": "mg/dL" },
                "status": "preliminary",
                "is_problem_list_item": false,
                "source_id": null,
                "encounter_id": null,
                "extracted_by": "manual",
                "model_version": null,
                "confidence": null,
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        reqwest::StatusCode::OK,
        "create observation with quantity value: {}",
        r.text().await.unwrap_or_default(),
    );

    // Re-fetch via the typed protocol type — this is the exact path that
    // would have failed if `Observation.value: Option<ObservationValue>`
    // (an untagged enum) couldn't round-trip a `Quantity` payload through
    // serde_json on the client side.
    let detail = transport
        .get_patient_detail(&connected, &access, patient_id)
        .await
        .expect("get_patient_detail after observation create");

    let r = admin
        .get_raw(&format!("/v1/patients/{patient_id_str}/observations"))
        .await
        .unwrap();
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    let obs_list_text = r.text().await.unwrap();
    let obs_list: Vec<
        anamnez_protocol::versioned::Versioned<anamnez_protocol::observation::Observation>,
    > = serde_json::from_str(&obs_list_text).expect("decode observations list");
    let lab = obs_list
        .iter()
        .find(|v| v.value.display_text == "[TEST] LDL Kolesterol")
        .expect("expected the LDL observation in the list");
    match &lab.value.value {
        Some(ObservationValue::Quantity(q)) => {
            assert_eq!(q.value, 130.0);
            assert_eq!(q.unit, "mg/dL");
        }
        other => panic!("expected Quantity value, got {other:?}"),
    }
    assert!(matches!(lab.value.status, ObservationStatus::Preliminary));
    assert_eq!(lab.value.code_system, Some(CodeSystem::AnamnezSym));
    assert_eq!(lab.value.code.as_deref(), Some("ANAMNEZ-SYM-LAB"));
    let _ = CodeSystem::Loinc;

    // The patient detail still loads cleanly after a lab observation is in
    // the system — same regression we test above, with extra payload now.
    assert_eq!(detail.encounters.len(), 1);

    // 9. logout cleans up.
    transport
        .logout(&connected, &access)
        .await
        .expect("logout");
}

