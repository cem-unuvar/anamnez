//! Cold-boot sequence: passphrase unwrap → DB open → audit chain verify (panic on
//! tamper) → load workstation deny-set → load TLS materials → build broadcast bus.

use crate::passphrase;
use crate::serve::app_state::AppState;
use anamnez_core::audit::verify::verify_chain;
use anamnez_core::config::Config;
use anamnez_core::db::Database;
use anamnez_core::error::{Error, Result};
use anamnez_core::ids::WorkstationId;
use anamnez_core::workstation;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

pub async fn cold_boot(cfg: Arc<Config>) -> Result<AppState> {
    let data_dir = cfg
        .db_path
        .parent()
        .ok_or(Error::Invariant("db_path has no parent directory"))?
        .to_path_buf();

    let pass = passphrase::unwrap_for(&data_dir)?;
    let db = Arc::new(Database::open(&cfg.db_path, pass, cfg.environment)?);

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
