//! Cold-boot sequence: passphrase unwrap → DB open → audit chain verify (panic on
//! tamper) → load workstation deny-set → load TLS materials → build broadcast bus.

use crate::serve::app_state::AppState;
use anamnez_core::audit::verify::verify_chain;
use anamnez_core::config::Config;
use anamnez_core::db::Database;
use anamnez_core::error::{Error, Result};
use anamnez_core::ids::WorkstationId;
use anamnez_core::key_custody::sep::MacosSep;
use anamnez_core::key_custody::ColdBoot;
use anamnez_core::workstation;
use parking_lot::RwLock;
use secrecy::SecretString;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

const ENV_RECOVERY_CODE: &str = "ANAMNEZ_RECOVERY_CODE";

pub async fn cold_boot(cfg: Arc<Config>) -> Result<AppState> {
    let data_dir = cfg
        .db_path
        .parent()
        .ok_or(Error::Invariant("db_path has no parent directory"))?
        .to_path_buf();

    let passphrase = unwrap_passphrase(&data_dir)?;
    let db = Arc::new(Database::open(&cfg.db_path, passphrase, cfg.environment)?);

    // Audit-chain verify — fatal on tamper.
    match verify_chain(&db) {
        Ok(report) => {
            tracing::info!(rows = report.rows_verified, "audit chain verified");
        }
        Err(Error::AuditTamper { row_id }) => {
            panic!("audit tampered at row {row_id}");
        }
        Err(e) => return Err(e),
    }

    let revoked: HashSet<WorkstationId> = workstation::list_revoked(&db)?.into_iter().collect();
    let revoked_devices = Arc::new(RwLock::new(revoked));

    let (events, _rx) = tokio::sync::broadcast::channel(256);

    Ok(AppState {
        db,
        events,
        revoked_devices,
        config: cfg,
        event_counter: Arc::new(AtomicU64::new(1)),
    })
}

/// Return the SQLCipher passphrase. SEP wrap is the default; recovery path is
/// activated by setting `ANAMNEZ_RECOVERY_CODE` — used by `anamnez init --restore`
/// after a hardware migration AND by layer-2 tests (which never go through SEP).
fn unwrap_passphrase(data_dir: &Path) -> Result<SecretString> {
    if let Ok(code) = std::env::var(ENV_RECOVERY_CODE) {
        let bytes = std::fs::read(data_dir.join("wrap_recovery.bin"))?;
        let cb = recovery_only_coldboot();
        return cb.unwrap_passphrase_via_recovery(&bytes, &SecretString::from(code));
    }
    // Production path — currently errors at `MacosSep::new()` until the Phase 6
    // SEP integration lands. Recovery path keeps the daemon bootable for tests.
    let sep = Arc::new(MacosSep::new()?);
    let cb = ColdBoot::new(sep);
    let bytes = std::fs::read(data_dir.join("wrap_sep.bin"))?;
    cb.unwrap_passphrase(&bytes)
}

/// `ColdBoot::unwrap_passphrase_via_recovery` does not actually use the SEP, but
/// the struct requires one at construction. We pass a never-used placeholder.
fn recovery_only_coldboot() -> ColdBoot {
    use anamnez_core::key_custody::SecureEnclaveWrap;
    struct Unused;
    impl SecureEnclaveWrap for Unused {
        fn wrap(&self, _: &SecretString) -> Result<Vec<u8>> {
            Err(Error::Invariant("SEP unused on recovery path"))
        }
        fn unwrap(&self, _: &[u8]) -> Result<SecretString> {
            Err(Error::Invariant("SEP unused on recovery path"))
        }
    }
    ColdBoot::new(Arc::new(Unused))
}

#[must_use]
pub fn tls_paths(data_dir: &Path) -> TlsPaths {
    TlsPaths {
        server_cert: data_dir.join("tls").join("server_cert.pem"),
        server_key: data_dir.join("tls").join("server_key.pem"),
        ca_cert: data_dir.join("tls").join("ca_cert.pem"),
    }
}

pub struct TlsPaths {
    pub server_cert: PathBuf,
    pub server_key: PathBuf,
    pub ca_cert: PathBuf,
}
