//! `TempDb` — tempdir + SQLCipher + refinery + env marker + code-systems CSV loaded.

use crate::db::Database;
use crate::env::Environment;
use crate::error::Result;
use crate::time::Clock;
use secrecy::SecretString;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// Fixed test SQLCipher passphrase. Trivial, since the DB is destroyed at end of test.
const TEST_PASSPHRASE: &str = "anamnez-test-passphrase-deterministic-7f9e1b3a";

pub struct TempDb {
    pub db: Database,
    pub root: TempDir,
    pub db_path: PathBuf,
}

impl TempDb {
    /// Open a fresh test DB in a temp directory. Default `Environment::Test`.
    pub fn new() -> Result<Self> {
        Self::new_with(Environment::Test, Arc::new(crate::time::SystemClock))
    }

    /// Open with a specific environment + clock.
    pub fn new_with(env: Environment, clock: Arc<dyn Clock>) -> Result<Self> {
        let root = TempDir::new()?;
        let db_path = root.path().join("anamnez.sqlite");
        let pass = SecretString::from(TEST_PASSPHRASE.to_owned());
        let db = Database::open_with_clock(&db_path, pass, env, clock)?;
        Ok(Self { db, root, db_path })
    }

    /// Path to the SQLite file inside the tempdir. Useful for tests that need to
    /// inspect/modify the file directly (e.g. cloning to test wrong-passphrase paths).
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.db_path
    }
}
