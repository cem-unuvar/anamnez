//! `clap` definitions for the `anamnez` binary. SPEC §Workspace structure.
//!
//! Subcommand surface mirrors the spec one-for-one:
//! `serve | init | migrate | backup | restore | audit verify | retention sweep |
//!  health | admin {add-user, disable-user, reset-password, enroll-workstation,
//!  breach-report, rotate-server-cert, update-codesystems}`.
//!
//! Concurrency rule (spec): write subcommands refuse to run while `serve` is up,
//! detected by a PID file. Read subcommands (`audit verify`, `backup`, `health`)
//! run alongside `serve` — SQLite WAL handles concurrent readers.

use clap::{Args, Parser, Subcommand};
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

    /// First-boot wizard: mint TLS materials + SQLCipher passphrase wraps + first admin.
    Init(InitArgs),

    /// Apply pending migrations to the DB. Forward-only.
    Migrate(MigrateArgs),

    /// Atomic encrypted snapshot of the live DB.
    Backup(BackupArgs),

    /// Replace the live DB with a snapshot file. Refuses while `serve` is up.
    Restore(RestoreArgs),

    /// Audit-log integrity operations.
    #[command(subcommand)]
    Audit(AuditCmd),

    /// Retention sweep operations.
    #[command(subcommand)]
    Retention(RetentionCmd),

    /// Process-level health probe (JSON to stdout).
    Health(HealthArgs),

    /// Admin operations.
    #[command(subcommand)]
    Admin(AdminCmd),
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: PathBuf,
    #[arg(long, env = "ANAMNEZ_BIND")]
    pub bind: Option<String>,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Where to lay down DB, wraps, TLS PEMs, and `config.toml`.
    #[arg(long)]
    pub data_dir: PathBuf,

    /// Admin email for the singleton-admin row.
    #[arg(long)]
    pub admin_email: String,

    /// Admin display name.
    #[arg(long)]
    pub admin_display_name: String,

    /// Environment marker baked into the DB. `production` | `test`.
    #[arg(long, default_value = "production")]
    pub environment: String,

    /// Host the server cert is issued for (SAN). Default `127.0.0.1`.
    #[arg(long, default_value = "127.0.0.1")]
    pub bind_host: String,

    /// Read admin password from stdin (first line) instead of prompting.
    #[arg(long)]
    pub password_stdin: bool,
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// If set, refuses to run while a live PID is recorded here.
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct BackupArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// Destination snapshot path. Will be overwritten.
    #[arg(long)]
    pub to: PathBuf,
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// Source snapshot path.
    #[arg(long)]
    pub from: PathBuf,
    /// Refuses while a live PID is recorded here.
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum AuditCmd {
    /// Verify the audit-log hash chain from the latest `retention_sweep` to head.
    Verify(AuditVerifyArgs),
}

#[derive(Debug, Args)]
pub struct AuditVerifyArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum RetentionCmd {
    /// Hard-delete rows past their horizon. Idempotent; nightly via `launchd`.
    Sweep(RetentionSweepArgs),
}

#[derive(Debug, Args)]
pub struct RetentionSweepArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct HealthArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum AdminCmd {
    /// Create a new user.
    AddUser(AdminAddUserArgs),

    /// Disable a user; transfer sole-ownership of patients to designated successors.
    DisableUser(AdminDisableUserArgs),

    /// Reset a user's password; revokes that user's active sessions.
    ResetPassword(AdminResetPasswordArgs),

    /// Issue a workstation enrollment — emits `anamnez://enroll?...`.
    EnrollWorkstation(AdminEnrollWorkstationArgs),

    /// Print the breach-scope report (CSV to stdout).
    BreachReport(AdminBreachReportArgs),

    /// Rotate the server TLS+CA keypair. Invalidates every workstation enrollment.
    RotateServerCert(AdminRotateServerCertArgs),

    /// Apply a signed code-systems bundle from a local file.
    UpdateCodesystems(AdminUpdateCodesystemsArgs),
}

#[derive(Debug, Args)]
pub struct AdminAddUserArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    #[arg(long)]
    pub email: String,
    #[arg(long)]
    pub display_name: String,
    /// `admin` | `provider`
    #[arg(long)]
    pub role: String,
    /// Read password from stdin (first line).
    #[arg(long)]
    pub password_stdin: bool,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AdminDisableUserArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// UUID of the target user.
    #[arg(long)]
    pub user: String,
    /// Repeatable `patient=<uuid>,user=<uuid>` — one per sole-owned patient.
    #[arg(long = "successor", value_parser = parse_successor)]
    pub successors: Vec<(String, String)>,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

fn parse_successor(s: &str) -> Result<(String, String), String> {
    let mut patient: Option<String> = None;
    let mut user: Option<String> = None;
    for part in s.split(',') {
        let (k, v) = part
            .split_once('=')
            .ok_or_else(|| format!("bad --successor segment `{part}`; expected key=value"))?;
        match k.trim() {
            "patient" => patient = Some(v.trim().to_owned()),
            "user" => user = Some(v.trim().to_owned()),
            other => return Err(format!("unknown --successor key `{other}`")),
        }
    }
    match (patient, user) {
        (Some(p), Some(u)) => Ok((p, u)),
        _ => Err("expected --successor patient=<uuid>,user=<uuid>".to_owned()),
    }
}

#[derive(Debug, Args)]
pub struct AdminResetPasswordArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// UUID of the target user. Mutually exclusive with --email.
    #[arg(long, conflicts_with = "email", required_unless_present = "email")]
    pub user: Option<String>,
    /// Email of the target user. Mutually exclusive with --user.
    #[arg(long, conflicts_with = "user", required_unless_present = "user")]
    pub email: Option<String>,
    #[arg(long)]
    pub password_stdin: bool,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AdminEnrollWorkstationArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    #[arg(long)]
    pub label: String,
    /// `bound` | `shared`
    #[arg(long, default_value = "bound")]
    pub mode: String,
    /// Email of the bound user (required if mode = bound).
    #[arg(long)]
    pub bind_user_email: Option<String>,
    /// LAN host the workstation client connects to. Embedded in the enroll URI.
    #[arg(long)]
    pub host: String,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AdminBreachReportArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// `auth_session.id` (UUID). Conflicts with --user/--since/--until.
    #[arg(long, conflicts_with_all = ["user", "since", "until"])]
    pub session: Option<String>,
    /// User UUID (requires --since and --until).
    #[arg(long, requires_all = ["since", "until"], conflicts_with = "session")]
    pub user: Option<String>,
    /// RFC 3339 start timestamp.
    #[arg(long)]
    pub since: Option<String>,
    /// RFC 3339 end timestamp.
    #[arg(long)]
    pub until: Option<String>,
}

#[derive(Debug, Args)]
pub struct AdminRotateServerCertArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// Host (SAN) the new server cert is issued for.
    #[arg(long, default_value = "127.0.0.1")]
    pub bind_host: String,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AdminUpdateCodesystemsArgs {
    #[arg(long, env = "ANAMNEZ_CONFIG")]
    pub config: PathBuf,
    /// Path to a signed bundle file (sidecar `.sig` expected next to it).
    #[arg(long)]
    pub from: PathBuf,
    #[arg(long, env = "ANAMNEZ_PID_FILE")]
    pub pid_file: Option<PathBuf>,
}
