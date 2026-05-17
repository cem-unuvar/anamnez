//! Helpers shared by every non-`serve` subcommand: TOML config load + DB open,
//! plus the PID-file refusal gate that write subcommands honor.

use crate::passphrase;
use crate::serve::config as serve_config;
use anamnez_core::config::Config;
use anamnez_core::db::Database;
use anamnez_core::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Load + validate the TOML config at `path`.
pub fn load_config(path: &Path) -> Result<Arc<Config>> {
    let cfg = serve_config::load(path)?;
    cfg.validate()?;
    Ok(Arc::new(cfg))
}

/// `data_dir` derived from `db_path.parent()`. Errors if missing.
pub fn data_dir(cfg: &Config) -> Result<PathBuf> {
    cfg.db_path
        .parent()
        .ok_or(Error::Invariant("db_path has no parent directory"))
        .map(Path::to_path_buf)
}

/// Open the SQLCipher DB the same way `serve` does (passphrase via env-var-or-DevSep).
pub fn open_db(cfg: &Config) -> Result<Database> {
    let dd = data_dir(cfg)?;
    let pass = passphrase::unwrap_for(&dd)?;
    Database::open(&cfg.db_path, pass, cfg.environment)
}

/// Refuse to proceed if `pid_file` points at a live process. No-op if the path
/// is absent (CLI invocation wasn't told to coordinate with `serve`). Stale
/// entries (path exists, PID is dead) are silently treated as unlocked.
pub fn refuse_while_serve_alive(pid_file: Option<&Path>) -> Result<()> {
    let Some(p) = pid_file else { return Ok(()) };
    if !p.exists() {
        return Ok(());
    }
    let prior = std::fs::read_to_string(p)?;
    let pid: i32 = prior
        .trim()
        .parse()
        .map_err(|_| Error::Invariant("pid file: malformed"))?;
    if pid_alive(pid) {
        Err(Error::Invariant(
            "another anamnez instance holds the pid file — stop `serve` first",
        ))
    } else {
        Ok(())
    }
}

fn pid_alive(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid), None).is_ok()
}
