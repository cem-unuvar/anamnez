//! Mac Studio binary — multi-tool with subcommands. SPEC §Workspace structure.
//!
//! `serve` is the long-running daemon driven by `tokio`. The rest of the
//! subcommands open the DB synchronously, do their work, and exit — they
//! never enter the async runtime.

use clap::Parser;
use std::process::ExitCode;

mod admin;
mod audit;
mod backup;
mod cli;
mod dispatch_common;
mod health;
mod init;
mod migrate;
mod passphrase;
mod restore;
mod retention;
mod serve;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    init_tracing();

    // Install the process-wide rustls crypto provider once at startup.
    // Multiple installs are idempotent (install_default returns Err on second call).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let res = match cli.cmd {
        cli::Cmd::Serve(args) => {
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
            rt.block_on(async move { serve::run(args).await })
        }
        cli::Cmd::Init(args) => init::run(args),
        cli::Cmd::Migrate(args) => migrate::run(args),
        cli::Cmd::Backup(args) => backup::run(args),
        cli::Cmd::Restore(args) => restore::run(args),
        cli::Cmd::Audit(cmd) => audit::run(cmd),
        cli::Cmd::Retention(cmd) => retention::run(cmd),
        cli::Cmd::Health(args) => health::run(args),
        cli::Cmd::Admin(cmd) => admin::run(cmd),
    };

    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "anamnez exited with error");
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
