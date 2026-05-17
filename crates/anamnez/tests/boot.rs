//! Layer-2: daemon boot path.

mod support;

use std::time::Duration;
use support::{bootstrap, spawn, tls};

#[tokio::test(flavor = "multi_thread")]
async fn boot_succeeds_with_fresh_state() {
    let bs = bootstrap::fresh();
    let port = spawn::pick_free_port();
    let bind = format!("127.0.0.1:{port}");
    let config_path = bs.data_dir.join("config.toml");
    let pid_path = bs.data_dir.join("anamnez.pid");
    let code_systems = bs.data_dir.join("code-systems");
    std::fs::create_dir_all(&code_systems).expect("code-systems dir");
    std::fs::create_dir_all(bs.data_dir.join("blobs")).expect("blobs dir");
    std::fs::write(
        &config_path,
        bootstrap::config_toml(&bs.data_dir, &code_systems),
    )
    .expect("write config");

    let mut daemon = spawn::spawn_serve(&config_path, &pid_path, &bind, &bs.recovery_code);

    let client = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    let base = format!("https://127.0.0.1:{port}");
    let ready = spawn::wait_for_ready(&client, &base, Duration::from_secs(15)).await;
    if !ready {
        // Bring back the exit code if the daemon died.
        if let Some(status) = daemon.try_wait().expect("try_wait") {
            panic!("daemon exited before becoming ready: {status}");
        }
        panic!("daemon did not become ready in 15s");
    }

    // Health endpoint returns 200 with status: ok.
    let resp = client
        .get(format!("{base}/v1/health"))
        .header("x-client-version", "1.0.0")
        .send()
        .await
        .expect("GET /v1/health");
    assert!(
        resp.status().is_success(),
        "health status: {}",
        resp.status()
    );
    let body: serde_json::Value = resp.json().await.expect("json");
    assert_eq!(body["status"], "ok");
}
