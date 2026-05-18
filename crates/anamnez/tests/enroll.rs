//! Layer 2: end-to-end enrollment exchange.
//!
//! Verifies:
//! - A no-cert client can only reach `/v1/enroll/exchange`. Every other route is
//!   rejected with `Forbidden` (the `require_device_id` middleware fires).
//! - `/v1/enroll/exchange` with a junk token returns `NotFound` — proves the route
//!   is reached without mTLS.
//! - End-to-end enroll: admin mints via `POST /v1/admin/workstations` →
//!   no-cert client redeems the token via `POST /v1/enroll/exchange` →
//!   a freshly-built client with the returned identity logs in successfully and
//!   the `LoginResponse` carries `environment` + `idle_lock_minutes`.

mod support;

use std::time::Duration;
use support::{api::Api, bootstrap, spawn, tls};

#[tokio::test(flavor = "multi_thread")]
async fn no_cert_client_cannot_reach_authed_routes() {
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
    let _daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);

    let warm = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    let ready = spawn::wait_for_ready(&warm, &base, Duration::from_secs(15)).await;
    assert!(ready, "daemon not ready");

    let no_cert = tls::client_no_identity(&bs.ca_pem);
    let r = no_cert
        .get(format!("{base}/v1/health"))
        .header("x-client-version", "1.0.0")
        .send()
        .await
        .expect("health request");
    assert_eq!(r.status(), reqwest::StatusCode::FORBIDDEN);

    let r = no_cert
        .post(format!("{base}/v1/auth/login"))
        .header("x-client-version", "1.0.0")
        .json(&serde_json::json!({
            "email": "a@b",
            "password": "x",
            "client_version": "1.0.0"
        }))
        .send()
        .await
        .expect("login request");
    assert_eq!(r.status(), reqwest::StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread")]
async fn enroll_exchange_with_junk_token_returns_404() {
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
    let _daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);

    let warm = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    assert!(spawn::wait_for_ready(&warm, &base, Duration::from_secs(15)).await);

    let no_cert = tls::client_no_identity(&bs.ca_pem);
    let r = no_cert
        .post(format!("{base}/v1/enroll/exchange"))
        .header("x-client-version", "1.0.0")
        .json(&serde_json::json!({
            "token": "deadbeef",
            "client_version": "1.0.0"
        }))
        .send()
        .await
        .expect("exchange request");
    // 404 NotFound — the route was reached (no mTLS rejection) and the token
    // lookup missed.
    assert_eq!(r.status(), reqwest::StatusCode::NOT_FOUND);
    let envelope: serde_json::Value = r.json().await.expect("error envelope");
    assert_eq!(envelope["kind"], "not_found");
}

#[tokio::test(flavor = "multi_thread")]
async fn end_to_end_enrollment_then_login_carries_environment() {
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
    let _daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);

    // 1) Admin logs in through the bootstrap workstation cert.
    let admin_client = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    assert!(spawn::wait_for_ready(&admin_client, &base, Duration::from_secs(15)).await);
    let mut admin = Api::new(admin_client, base.clone());
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
        .expect("admin login");
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    let login: serde_json::Value = r.json().await.unwrap();
    admin.bearer = Some(login["access_token"].as_str().unwrap().to_owned());
    assert_eq!(login["environment"], "test");
    assert!(login["idle_lock_minutes"].is_number());

    // 2) Admin mints an enrollment via the HTTP admin route. Step-up requires
    //    the password to be re-entered as the `X-Step-Up-Password` header.
    admin.stepup_password = Some(bs.admin_password.clone());
    let r = admin
        .post_raw(
            "/v1/admin/workstations",
            &serde_json::json!({
                "label": "Test Bench",
                "mode": "shared",
                "host": "127.0.0.1",
            }),
        )
        .await
        .expect("mint enrollment");
    assert_eq!(r.status(), reqwest::StatusCode::OK, "mint status");
    let minted: serde_json::Value = r.json().await.expect("mint json");
    let token = minted["token"].as_str().expect("token").to_owned();
    assert!(minted["uri"]
        .as_str()
        .unwrap()
        .starts_with("anamnez://enroll?host="));

    // 3) A no-cert client redeems the token via /v1/enroll/exchange.
    let no_cert = tls::client_no_identity(&bs.ca_pem);
    let r = no_cert
        .post(format!("{base}/v1/enroll/exchange"))
        .header("x-client-version", "1.0.0")
        .json(&serde_json::json!({
            "token": token,
            "client_version": "1.0.0",
        }))
        .send()
        .await
        .expect("exchange");
    assert_eq!(r.status(), reqwest::StatusCode::OK, "exchange status");
    let exchanged: serde_json::Value = r.json().await.unwrap();
    let client_cert_pem = exchanged["client_cert_pem"].as_str().unwrap().to_owned();
    let client_key_pem = exchanged["client_key_pem"].as_str().unwrap().to_owned();
    let ca_cert_pem = exchanged["ca_cert_pem"].as_str().unwrap().to_owned();
    assert!(client_cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(client_key_pem.contains("PRIVATE KEY"));
    assert_eq!(ca_cert_pem, bs.ca_pem);

    // 4) A fresh client built from the returned identity can hit /v1/health and
    //    log in. The mTLS handshake succeeds against the newly-minted workstation.
    let new_ws = tls::client(&ca_cert_pem, &client_cert_pem, &client_key_pem);
    let r = new_ws
        .get(format!("{base}/v1/health"))
        .header("x-client-version", "1.0.0")
        .send()
        .await
        .expect("health");
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    let health: serde_json::Value = r.json().await.unwrap();
    assert_eq!(health["environment"], "test");

    let r = new_ws
        .post(format!("{base}/v1/auth/login"))
        .header("x-client-version", "1.0.0")
        .json(&serde_json::json!({
            "email": bs.admin_email,
            "password": bs.admin_password,
            "client_version": "1.0.0",
        }))
        .send()
        .await
        .expect("login via new workstation");
    assert_eq!(r.status(), reqwest::StatusCode::OK);
}
