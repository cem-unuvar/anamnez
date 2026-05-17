//! README §Storage → Engine — "Schema-version mismatch on startup means the server refuses to boot."

use crate::error::{Error, Result};

/// Most recent migration version embedded in this binary.
pub const BINARY_SCHEMA_VERSION: u32 = 5;

/// Refuses to boot with `Error::SchemaVersionMismatch` if the DB's applied version
/// is not exactly `BINARY_SCHEMA_VERSION`. Called after migrations have run.
pub fn assert_matches(conn: &mut rusqlite::Connection) -> Result<()> {
    let applied = super::migrations::latest_applied(conn)?.unwrap_or(0);
    if applied != BINARY_SCHEMA_VERSION {
        return Err(Error::SchemaVersionMismatch {
            db: applied.to_string(),
            binary: BINARY_SCHEMA_VERSION.to_string(),
        });
    }
    Ok(())
}
