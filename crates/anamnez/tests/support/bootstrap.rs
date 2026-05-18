//! Calls `anamnez_core::bootstrap::run` with fixture SEP to lay down on-disk state.

use super::cert_mint;
use anamnez_core::bootstrap::{run, BootstrapArtifacts, BootstrapInputs};
use anamnez_core::env::Environment;
use anamnez_core::ids::WorkstationId;
use anamnez_core::rng::OsRng;
use anamnez_core::test_support::sep::FixtureSep;
use anamnez_core::workstation;
use secrecy::{ExposeSecret, SecretString};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

pub struct Bootstrapped {
    pub tempdir: TempDir,
    pub data_dir: PathBuf,
    pub admin_email: String,
    pub admin_password: String,
    pub recovery_code: String,
    pub ca_pem: String,
    pub server_cert_pem: String,
    pub server_key_pem: String,
    pub workstation_id: WorkstationId,
    pub workstation_cert_pem: String,
    pub workstation_key_pem: String,
    pub artifacts: BootstrapArtifacts,
}

pub fn fresh() -> Bootstrapped {
    let temp = TempDir::new().expect("tempdir");
    let data_dir = temp.path().to_path_buf();

    // Mint TLS materials.
    let ca = cert_mint::mint_ca();
    let server = cert_mint::mint_server_cert(&ca, "127.0.0.1");

    let admin_email = "admin@example.test".to_owned();
    let admin_password = "[TEST]-correct-horse-battery-staple".to_owned();

    let artifacts = run(BootstrapInputs {
        data_dir: &data_dir,
        environment: Environment::Test,
        admin_email: admin_email.clone(),
        admin_password: SecretString::from(admin_password.clone()),
        admin_display_name: "Test Admin".to_owned(),
        sep: Arc::new(FixtureSep::new()),
        rng: Arc::new(OsRng),
        server_cert_pem: server.cert_pem.clone(),
        server_key_pem: server.key_pem.clone(),
        ca_cert_pem: ca.ca_cert_pem.clone(),
        ca_key_pem: ca.ca_key_pair.serialize_pem(),
    })
    .expect("bootstrap::run");

    // Mint a workstation cert and INSERT a row into the workstation table so
    // mTLS handshake passes (the CN must match a non-revoked workstation row).
    let workstation_id = WorkstationId::new();
    let ws = cert_mint::mint_workstation_cert(&ca, workstation_id.as_uuid());

    // Insert workstation row + ensure admin has a session-compatible binding.
    insert_workstation(&data_dir, workstation_id, artifacts.admin_user_id).expect("insert ws");

    let recovery = artifacts.recovery_code.expose_secret().to_owned();

    Bootstrapped {
        tempdir: temp,
        data_dir,
        admin_email,
        admin_password,
        recovery_code: recovery,
        ca_pem: ca.ca_cert_pem,
        server_cert_pem: server.cert_pem,
        server_key_pem: server.key_pem,
        workstation_id,
        workstation_cert_pem: ws.cert_pem,
        workstation_key_pem: ws.key_pem,
        artifacts,
    }
}

fn insert_workstation(
    data_dir: &std::path::Path,
    id: WorkstationId,
    admin: anamnez_core::ids::UserId,
) -> anamnez_core::error::Result<()> {
    // Open the freshly-bootstrapped DB via the recovery path to insert the workstation.
    let wrap_recovery = std::fs::read(data_dir.join("wrap_recovery.bin"))?;
    let cb = anamnez_core::key_custody::ColdBoot::new(Arc::new(FixtureSep::new()));
    // FixtureSep can also wrap/unwrap directly; use the recovery path so we don't
    // need to read the recovery code (we wrote both wraps from the same pass).
    let _ = wrap_recovery; // FixtureSep::unwrap on wrap_sep is simpler:
    let wrap_sep = std::fs::read(data_dir.join("wrap_sep.bin"))?;
    let passphrase = cb.unwrap_passphrase(&wrap_sep)?;

    let db = anamnez_core::db::Database::open(
        &data_dir.join("anamnez.sqlite"),
        passphrase,
        Environment::Test,
    )?;
    workstation::enroll(
        &db,
        admin,
        workstation::NewWorkstation {
            label: "Test Workstation".to_owned(),
            mode: workstation::Mode::Shared,
            bound_user_id: None,
            cert_serial: format!("test-{}", id.as_uuid()),
            cert_fingerprint: format!("test-fp-{}", id.as_uuid()),
        },
    )?;
    // Workstation row uses a freshly-generated id; rewrite to match our minted cert CN.
    db.with_writer(|conn| {
        conn.execute(
            "UPDATE workstation SET id = ?1 WHERE cert_serial = ?2",
            rusqlite::params![id.as_uuid().to_string(), format!("test-{}", id.as_uuid())],
        )?;
        Ok(())
    })?;
    Ok(())
}

pub fn config_toml(data_dir: &std::path::Path, code_systems_root: &std::path::Path) -> String {
    format!(
        r#"
environment = "test"
db_path = "{}/anamnez.sqlite"
blob_root = "{}/blobs"
idle_lock_minutes = 10
code_systems_root = "{}"

[min_client_version]
major = 1
minor = 0
patch = 0
"#,
        data_dir.display(),
        data_dir.display(),
        code_systems_root.display(),
    )
}
