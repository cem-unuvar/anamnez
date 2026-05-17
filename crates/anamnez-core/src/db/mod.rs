//! README §Storage → Engine — SQLite (SQLCipher) embedded in the server process.
//!
//! Single writer + reader pool. Pragmas (`journal_mode = WAL`, `foreign_keys = ON`,
//! `STRICT` tables) are enforced at every open.

pub mod env_marker;
pub mod migrations;
pub mod pool;
pub mod pragmas;
pub mod schema_version;

use crate::env::Environment;
use crate::error::{Error, Result};
use crate::time::{Clock, SystemClock};
use parking_lot::Mutex;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use secrecy::SecretString;
use std::path::Path;
use std::sync::Arc;

/// Top-level handle over an open anamnez database.
pub struct Database {
    writer: Mutex<Connection>,
    readers: r2d2::Pool<SqliteConnectionManager>,
    clock: Arc<dyn Clock>,
    daemon_env: Environment,
}

impl Database {
    /// Open or create an anamnez DB at `path` with the given SQLCipher passphrase.
    ///
    /// Order of operations: open writer → pragmas (incl. passphrase) → migrations →
    /// env-marker (write on first boot, otherwise assert match) → schema-version check →
    /// build reader pool.
    pub fn open(path: &Path, passphrase: SecretString, env: Environment) -> Result<Self> {
        Self::open_with_clock(path, passphrase, env, Arc::new(SystemClock))
    }

    /// Same as [`Self::open`] but with an injectable `Clock` (used by tests).
    pub fn open_with_clock(
        path: &Path,
        passphrase: SecretString,
        env: Environment,
        clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        let mut writer = pool::open_writer(path, &passphrase)?;

        // Probe the passphrase: SQLCipher only fails on the first I/O after `PRAGMA key`.
        probe_passphrase(&writer)?;

        migrations::apply(&mut writer)?;
        schema_version::assert_matches(&mut writer)?;
        env_marker::read_or_init(&writer, env, clock.now())?;

        // Pragmas applied at open are sufficient for the writer; assert as a safety net.
        pragmas::assert(&writer)?;

        let readers = pool::build_reader_pool(path, passphrase, pool::DEFAULT_READER_POOL_SIZE)?;

        Ok(Self {
            writer: Mutex::new(writer),
            readers,
            clock,
            daemon_env: env,
        })
    }

    /// The environment the daemon is running as.
    #[must_use]
    pub fn env(&self) -> Environment {
        self.daemon_env
    }

    /// Borrow the injected clock.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// Run an operation under the single writer connection.
    pub fn with_writer<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Connection) -> Result<R>,
    {
        let mut guard = self.writer.lock();
        f(&mut guard)
    }

    /// Borrow a reader connection from the pool.
    pub fn with_reader<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let conn = self
            .readers
            .get()
            .map_err(|e| Error::Invariant(string_leak(&format!("reader checkout: {e}"))))?;
        f(&conn)
    }

    /// Direct access to a pooled reader connection. Prefer [`Self::with_reader`].
    pub fn checkout_reader(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.readers
            .get()
            .map_err(|e| Error::Invariant(string_leak(&format!("reader checkout: {e}"))))
    }
}

fn probe_passphrase(conn: &Connection) -> Result<()> {
    // Any read against an encrypted DB with the wrong passphrase fails here.
    conn.query_row("SELECT 1 FROM sqlite_schema LIMIT 1", [], |_| Ok(()))
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(()),
            rusqlite::Error::SqliteFailure(_, _) => Err(Error::BadCredentials),
            other => Err(Error::from(other)),
        })
}

fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
