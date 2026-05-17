//! SPEC §Storage → Backups — `sqlite3_backup_*` online snapshot.
//!
//! `take()` opens the destination as a fresh SQLCipher DB with the supplied
//! passphrase and streams pages via SQLite's online backup API. The live DB is
//! not blocked: the backup API is concurrent-reader-safe and serializes against
//! the single writer for short bursts.
//!
//! `restore()` is the symmetric inverse — it overwrites the live DB file with
//! the snapshot after probing that the snapshot opens under the live
//! passphrase. Callers (CLI) enforce that `serve` is not running before
//! invoking `restore()`; that check lives at the binary boundary, not here.
//!
//! The audit-action enum has no `backup.*` variant (SPEC closed enum, adding
//! one requires a spec PR), so neither operation appends an audit row. The
//! CLI logs the action to stdout for operator visibility.

use crate::db::Database;
use crate::error::{Error, Result};
use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use secrecy::{ExposeSecret, SecretString};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BackupReport {
    pub pages_copied: i32,
    pub dst_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct RestoreReport {
    pub bytes_copied: u64,
}

/// Atomic encrypted snapshot of `db` to `dst`. `dst_passphrase` becomes the new
/// snapshot's SQLCipher passphrase — typically the same as the live DB so a
/// recovery-restore opens it directly.
pub fn take(db: &Database, dst: &Path, dst_passphrase: &SecretString) -> Result<BackupReport> {
    if let Some(parent) = dst.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    if dst.exists() {
        std::fs::remove_file(dst)?;
    }

    let mut dst_conn = Connection::open_with_flags(
        dst,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    crate::db::pragmas::apply(&dst_conn, dst_passphrase.expose_secret())?;

    let pages_copied = db.with_writer(|src| {
        let backup = Backup::new(src, &mut dst_conn)?;
        backup.run_to_completion(256, Duration::from_millis(0), None)?;
        Ok::<i32, Error>(backup.progress().pagecount)
    })?;

    Ok(BackupReport {
        pages_copied,
        dst_bytes: std::fs::metadata(dst).map(|m| m.len()).unwrap_or(0),
    })
}

/// Replace the live DB at `db_path` with the snapshot at `src`. Probes that the
/// snapshot opens under `live_passphrase` before clobbering anything; refuses
/// loudly on mismatch (`Error::BadCredentials`).
pub fn restore(
    src: &Path,
    db_path: &Path,
    live_passphrase: &SecretString,
) -> Result<RestoreReport> {
    let probe = Connection::open_with_flags(
        src,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    crate::db::pragmas::apply(&probe, live_passphrase.expose_secret())?;
    probe
        .query_row("SELECT 1 FROM sqlite_schema LIMIT 1", [], |_| Ok(()))
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(()),
            rusqlite::Error::SqliteFailure(_, _) => Err(Error::BadCredentials),
            other => Err(Error::from(other)),
        })?;
    drop(probe);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    for ext in ["-wal", "-shm"] {
        let mut p = db_path.as_os_str().to_owned();
        p.push(ext);
        let sidecar = std::path::PathBuf::from(p);
        if sidecar.exists() {
            let _ = std::fs::remove_file(&sidecar);
        }
    }
    std::fs::copy(src, db_path)?;
    Ok(RestoreReport {
        bytes_copied: std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0),
    })
}
