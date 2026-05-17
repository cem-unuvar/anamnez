//! Layer-2: step-up reauthentication gates on admin write endpoints.

mod support;

use std::time::Duration;
use support::{api::Api, bootstrap, spawn, tls};

#[tokio::test(flavor = "multi_thread")]
async fn user_create_without_stepup_password_returns_401() {
    let (_d, api) = setup().await;
    let body = serde_json::json!({
        "email": "newuser@example.test",
        "display_name": "[TEST] New User",
        "role": "provider",
        "password": "[TEST]-some-strong-password",
    });
    let resp = api.post_raw("/v1/admin/users", &body).await.unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "want 401 without X-Step-Up-Password header"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn user_create_with_correct_stepup_password_succeeds() {
    let (_d, mut api) = setup().await;
    api.stepup_password = Some(api_admin_password());
    let body = serde_json::json!({
        "email": "newuser2@example.test",
        "display_name": "[TEST] New User Two",
        "role": "provider",
        "password": "[TEST]-some-strong-password",
    });
    let resp = api.post_raw("/v1/admin/users", &body).await.unwrap();
    assert!(
        resp.status().is_success(),
        "want 200 with correct step-up password, got {} body: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
}

fn api_admin_password() -> String {
    "[TEST]-correct-horse-battery-staple".to_owned()
}

async fn setup() -> (spawn::Daemon, Api) {
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

    let daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);
    let client = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    let ready = spawn::wait_for_ready(&client, &base, Duration::from_secs(15)).await;
    assert!(ready, "daemon not ready");

    let mut api = Api::new(client, base);
    let body = serde_json::json!({
        "email": bs.admin_email,
        "password": bs.admin_password,
        "client_version": "1.0.0",
    });
    let resp = api.post_raw("/v1/auth/login", &body).await.unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    api.bearer = Some(json["access_token"].as_str().unwrap().to_owned());
    (daemon, api)
}
