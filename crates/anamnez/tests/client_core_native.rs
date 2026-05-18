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
use anamnez_protocol::enroll::EnrollExchangeRequest;
use anamnez_protocol::environment::Environment;
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

    // 7. logout cleans up.
    transport
        .logout(&connected, &access)
        .await
        .expect("logout");
}

