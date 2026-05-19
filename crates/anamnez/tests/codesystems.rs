//! Layer-2: `GET /v1/codesystems/search` autocomplete round-trip. Pre-seeds the
//! daemon's DB with `ANAMNEZ-SYM` from the repo CSV, spawns serve, and hits the
//! route over real TLS+auth.

mod support;

use std::sync::Arc;
use std::time::Duration;
use support::{api::Api, bootstrap, spawn, tls};

#[tokio::test(flavor = "multi_thread")]
async fn autocomplete_round_trip() {
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

    seed_anamnez_sym(&bs.data_dir);

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

    // Happy path: lowercase ASCII subset of "boyun ağrısı" — must hit at least one row.
    let resp = api
        .get_raw("/v1/codesystems/search?system=ANAMNEZ-SYM&q=boyun")
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().expect("hits array");
    assert!(!hits.is_empty(), "expected at least one hit for `boyun`");
    let display_trs: Vec<String> = hits
        .iter()
        .filter_map(|h| h["display_tr"].as_str().map(str::to_owned))
        .collect();
    assert!(
        display_trs.iter().any(|s| s.contains("boyun")),
        "no hit with `boyun` in display_tr; got {display_trs:?}"
    );

    // Turkish casefold: uppercase query must return the same hits as lowercase.
    let resp = api
        .get_raw("/v1/codesystems/search?system=ANAMNEZ-SYM&q=BOYUN")
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body_upper: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body_upper["hits"], body["hits"], "casefold parity broken");

    // Empty query → 200, empty hits.
    let resp = api
        .get_raw("/v1/codesystems/search?system=ANAMNEZ-SYM&q=")
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["hits"].as_array().unwrap().len(), 0);

    // Invalid system tag → 400 from axum's query rejection.
    let resp = api
        .get_raw("/v1/codesystems/search?system=NOPE&q=anything")
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Limit clamping: very large limit is accepted (clamped server-side, not rejected).
    let resp = api
        .get_raw("/v1/codesystems/search?system=ANAMNEZ-SYM&q=boyun&limit=9999")
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

/// Open the freshly-bootstrapped DB and load the ANAMNEZ-SYM CSV. The DB handle
/// is dropped before the daemon spawns so SQLCipher serialization is clean.
fn seed_anamnez_sym(data_dir: &std::path::Path) {
    use anamnez_core::code_systems::{loader, repo_code_systems_root, CodeSystem};
    use anamnez_core::env::Environment;
    use anamnez_core::key_custody::ColdBoot;
    use anamnez_core::test_support::sep::FixtureSep;

    let cb = ColdBoot::new(Arc::new(FixtureSep::new()));
    let wrap_sep = std::fs::read(data_dir.join("wrap_sep.bin")).expect("read wrap_sep.bin");
    let passphrase = cb.unwrap_passphrase(&wrap_sep).expect("unwrap passphrase");
    let db = anamnez_core::db::Database::open(
        &data_dir.join("anamnez.sqlite"),
        passphrase,
        Environment::Test,
    )
    .expect("open DB");

    // ANAMNEZ-SYM references ICD-10-TM via icd10_suggestion FK, so load ICD-10-TM first.
    let root = repo_code_systems_root();
    db.with_writer(|conn| loader::load_csv(conn, CodeSystem::Icd10Tm, &root.join("icd10-tm/normalized.csv")))
        .expect("load ICD-10-TM");
    db.with_writer(|conn| {
        loader::load_csv(
            conn,
            CodeSystem::AnamnezSym,
            &root.join("anamnez-sym/normalized.csv"),
        )
    })
    .expect("load ANAMNEZ-SYM");
}
