//! `anamnez serve` — long-running HTTPS daemon. SPEC §Workstation client → Wire protocol.

pub mod app_state;
pub mod boot;
pub mod config;
pub mod error;
pub mod middleware;
pub mod mtls;
pub mod pid_file;
pub mod routes;
pub mod sse;
pub mod tls_serve;

use crate::cli::ServeArgs;
use anamnez_core::error::Result;
use std::net::SocketAddr;
use std::str::FromStr;

/// Entry point — wires every subsystem and runs the daemon to completion or
/// graceful shutdown.
pub async fn run(args: ServeArgs) -> Result<()> {
    // PID file first: refuses to start if another `anamnez serve` (or other write
    // subcommand) holds it.
    let _pid_guard = pid_file::acquire(&args.pid_file)?;

    let cfg = config::load(&args.config)?;
    cfg.validate()?;
    let cfg = std::sync::Arc::new(cfg);

    let state = boot::cold_boot(cfg.clone()).await?;

    let bind: String = args
        .bind
        .clone()
        .or_else(|| std::env::var("ANAMNEZ_BIND").ok())
        .unwrap_or_else(|| "127.0.0.1:8443".to_owned());
    let addr = SocketAddr::from_str(&bind)
        .map_err(|_| anamnez_core::Error::Invariant("bind address parse failed"))?;

    tracing::info!(%addr, "anamnez serve listening (mTLS)");
    tls_serve::run(addr, state).await
}
