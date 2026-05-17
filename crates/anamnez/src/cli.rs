//! `clap` definitions for the `anamnez` binary.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "anamnez",
    version,
    about = "anamnez Mac Studio appliance binary"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Run the long-running HTTPS daemon.
    Serve(ServeArgs),
}

#[derive(Debug, Parser)]
pub struct ServeArgs {
    /// Path to the daemon's TOML config file.
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,

    /// Path to the PID file (set on startup; removed at shutdown).
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: PathBuf,

    /// Bind address (overrides config). Default: read from config.
    #[arg(long, env = "ANAMNEZ_BIND")]
    pub bind: Option<String>,
}
