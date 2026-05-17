//! README §Privacy — production-vs-test marker enforcement.

use crate::env::Environment;
use crate::error::{Error, Result};
use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension};

/// Insert the marker on first boot. Errors if a marker already exists (the
/// `BEFORE UPDATE/DELETE` trigger plus `singleton = 1` PK guarantee one row max).
pub fn write_first_boot(conn: &Connection, env: Environment, now: Timestamp) -> Result<()> {
    conn.execute(
        "INSERT INTO environment_marker (singleton, environment, written_at) VALUES (1, ?1, ?2)",
        params![env.as_str(), now.to_string()],
    )?;
    Ok(())
}

/// Read the marker. Returns `Error::Invariant` if missing or unparseable.
pub fn read(conn: &Connection) -> Result<Environment> {
    let s: Option<String> = conn
        .query_row(
            "SELECT environment FROM environment_marker WHERE singleton = 1",
            params![],
            |row| row.get(0),
        )
        .optional()?;
    let s = s.ok_or(Error::Invariant("environment marker missing"))?;
    Environment::from_marker_str(&s).ok_or(Error::Invariant("environment marker has unknown value"))
}

/// Read-or-init: if marker is absent, write the daemon's environment; if present,
/// assert it matches and fail loudly otherwise.
pub fn read_or_init(conn: &Connection, daemon_env: Environment, now: Timestamp) -> Result<()> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT environment FROM environment_marker WHERE singleton = 1",
            params![],
            |row| row.get(0),
        )
        .optional()?;
    match existing {
        None => write_first_boot(conn, daemon_env, now),
        Some(s) => {
            let db_env = Environment::from_marker_str(&s)
                .ok_or(Error::Invariant("environment marker has unknown value"))?;
            if db_env != daemon_env {
                return Err(Error::EnvironmentMarkerMismatch {
                    db: db_env.as_str().to_owned(),
                    daemon: daemon_env.as_str().to_owned(),
                });
            }
            Ok(())
        }
    }
}
