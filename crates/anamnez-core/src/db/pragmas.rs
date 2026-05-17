//! README §Storage → Engine — non-default pragmas asserted on every connection.

use crate::error::{Error, Result};
use rusqlite::{params, Connection};

/// Apply the anamnez pragma set to a freshly opened connection.
///
/// Order matters: `PRAGMA key` must run before any other statement so that subsequent
/// pragmas (`journal_mode`, `foreign_keys`) execute against the decrypted database.
/// `cipher_compatibility = 4` matches SQLCipher 4 defaults; making it explicit prevents
/// silent upgrades on future bumps.
pub fn apply(conn: &Connection, passphrase: &str) -> Result<()> {
    // SQLCipher passphrase — must be first.
    conn.pragma_update(None, "key", passphrase)?;
    conn.pragma_update(None, "cipher_compatibility", 4)?;

    // Write-ahead logging for concurrent readers against a single writer.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    // Foreign keys are off by default in SQLite — turn them on for every connection.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // 5-second busy timeout; well above expected WAL contention.
    conn.pragma_update(None, "busy_timeout", 5_000)?;
    // Reasonable defaults for an embedded clinical DB.
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;

    Ok(())
}

/// Assert that the pragmas we care about are set as expected on `conn`.
/// Used by tests and at runtime on writer-connection acquisition.
pub fn assert(conn: &Connection) -> Result<()> {
    let journal_mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(Error::Invariant("journal_mode must be WAL"));
    }
    let fk: i64 = conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if fk != 1 {
        return Err(Error::Invariant("foreign_keys must be ON"));
    }
    assert_strict_tables(conn)?;
    Ok(())
}

/// Walks `sqlite_schema` and asserts every user table is declared `STRICT`.
fn assert_strict_tables(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT name, sql FROM sqlite_schema \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
           AND name NOT LIKE 'refinery_%' \
           AND name NOT LIKE 'fts_%' \
           AND sql IS NOT NULL",
    )?;
    let rows = stmt.query_map(params![], |row| {
        let name: String = row.get(0)?;
        let sql: String = row.get(1)?;
        Ok((name, sql))
    })?;
    for row in rows {
        let (name, sql) = row?;
        if !sql.to_ascii_uppercase().contains("STRICT") {
            tracing::error!("non-strict table detected: {}", name);
            return Err(Error::Invariant("all user tables must be STRICT"));
        }
    }
    Ok(())
}
