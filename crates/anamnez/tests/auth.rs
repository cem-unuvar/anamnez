//! Layer-2: auth lifecycle + outdated-client floor.

mod support;

use std::time::Duration;
use support::{api::Api, bootstrap, spawn, tls};

async fn spawn_and_login() -> (
    spawn::Daemon,
    Api,
    bootstrap::Bootstrapped,
    String, /* base url */
) {
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

    let mut api = Api::new(client, base.clone());
    // Log in as the bootstrap-minted admin.
    let body = serde_json::json!({
        "email": bs.admin_email,
        "password": bs.admin_password,
        "client_version": "1.0.0",
    });
    let resp = api.post_raw("/v1/auth/login", &body).await.expect("login");
    assert_eq!(resp.status(), reqwest::StatusCode::OK, "login");
    let json: serde_json::Value = resp.json().await.expect("login json");
    let token = json["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned();
    api.bearer = Some(token);
    (daemon, api, bs, base)
}

#[tokio::test(flavor = "multi_thread")]
async fn login_succeeds_with_admin_credentials() {
    let (_d, _api, _bs, _base) = spawn_and_login().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn outdated_client_returns_426() {
    let bs = bootstrap::fresh();
    let port = spawn::pick_free_port();
    let bind = format!("127.0.0.1:{port}");
    let base = format!("https://127.0.0.1:{port}");
    let config_path = bs.data_dir.join("config.toml");
    let pid_path = bs.data_dir.join("anamnez.pid");
    let code_systems = bs.data_dir.join("code-systems");
    std::fs::create_dir_all(&code_systems).unwrap();
    std::fs::create_dir_all(bs.data_dir.join("blobs")).unwrap();

    // Config requires min version 2.0.0.
    let toml = format!(
        r#"
environment = "test"
db_path = "{}/anamnez.sqlite"
blob_root = "{}/blobs"
idle_lock_minutes = 10
code_systems_root = "{}"

[min_client_version]
major = 2
minor = 0
patch = 0
"#,
        bs.data_dir.display(),
        bs.data_dir.display(),
        code_systems.display(),
    );
    std::fs::write(&config_path, toml).unwrap();
    let _daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);
    let client = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );

    // Wait for daemon to be ready — use version 2.0.0 for the readiness probe so
    // it doesn't get gated out itself.
    let start = std::time::Instant::now();
    let mut ready = false;
    while start.elapsed() < Duration::from_secs(15) {
        if let Ok(r) = client
            .get(format!("{base}/v1/health"))
            .header("x-client-version", "2.0.0")
            .send()
            .await
        {
            if r.status().is_success() {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "daemon not ready");

    // Now hit it with an outdated client version.
    let resp = client
        .get(format!("{base}/v1/health"))
        .header("x-client-version", "1.0.0")
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status(), reqwest::StatusCode::UPGRADE_REQUIRED);
}
