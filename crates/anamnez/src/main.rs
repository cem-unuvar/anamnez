//! Mac Studio binary — multi-tool with subcommands. SPEC §Workspace structure.
//!
//! Subsystem H lands the `serve` subcommand. Other subcommands (`init`, `migrate`,
//! `backup`, `restore`, `audit verify`, `retention sweep`, `health`, `admin *`)
//! land in later slices.

use clap::Parser;
use std::process::ExitCode;

mod cli;
mod serve;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    init_tracing();

    // Install the process-wide rustls crypto provider once at startup.
    // Multiple installs are idempotent (install_default returns Err on second call).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("anamnez: failed to build tokio runtime: {e}");
            return ExitCode::from(1);
        }
    };

    let res = rt.block_on(async move {
        match cli.cmd {
            cli::Cmd::Serve(args) => serve::run(args).await,
        }
    });

    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "anamnez serve exited with error");
            eprintln!("anamnez: {e}");
            ExitCode::from(1)
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,anamnez=info"));
    fmt().with_env_filter(filter).with_target(false).init();
}
