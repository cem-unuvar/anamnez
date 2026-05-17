//! SPEC §Development — write subcommands refuse to run while `serve` holds the
//! PID file. `serve` writes the file on start; the `Drop` impl removes it.

use anamnez_core::error::{Error, Result};
use std::path::{Path, PathBuf};

pub struct PidGuard {
    path: PathBuf,
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Acquire the PID file. If a stale entry is present (file exists, but PID is
/// no longer alive), overwrites it. If the live PID at the file is alive, errors.
pub fn acquire(path: &Path) -> Result<PidGuard> {
    if path.exists() {
        let prior = std::fs::read_to_string(path).map_err(Error::from)?;
        let pid: i32 = prior
            .trim()
            .parse()
            .map_err(|_| Error::Invariant("pid file: malformed"))?;
        if pid_alive(pid) {
            return Err(Error::Invariant("pid file: another instance is running"));
        }
        // Stale — fall through and overwrite.
        tracing::warn!(stale_pid = pid, path = ?path, "overwriting stale pid file");
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(Error::from)?;
        }
    }
    std::fs::write(path, format!("{}", std::process::id())).map_err(Error::from)?;
    Ok(PidGuard {
        path: path.to_path_buf(),
    })
}

fn pid_alive(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid), None).is_ok()
}
