//! Layer-2: mTLS rejection paths.

mod support;

use std::time::Duration;
use support::{bootstrap, cert_mint, spawn, tls};

#[tokio::test(flavor = "multi_thread")]
async fn mtls_rejects_client_cert_from_unknown_ca() {
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

    // Trust the daemon's CA but present a client cert signed by an unrelated CA.
    let other_ca = cert_mint::mint_ca();
    let bad_ws = cert_mint::mint_workstation_cert(&other_ca, uuid::Uuid::new_v4());
    let client = tls::client(&bs.ca_pem, &bad_ws.cert_pem, &bad_ws.key_pem);

    // Wait briefly so the daemon is bound. Use the legitimate client to verify
    // readiness, then switch to the bad-cert client.
    let warm = tls::client(
        &bs.ca_pem,
        &bs.workstation_cert_pem,
        &bs.workstation_key_pem,
    );
    let ready = spawn::wait_for_ready(&warm, &base, Duration::from_secs(15)).await;
    assert!(ready, "daemon not ready");

    let res = client
        .get(format!("{base}/v1/health"))
        .header("x-client-version", "1.0.0")
        .send()
        .await;
    assert!(
        res.is_err(),
        "expected TLS handshake rejection; got {:?}",
        res.as_ref().map(|r| r.status())
    );
}
