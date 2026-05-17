//! README §Storage → Engine — refinery, versioned and forward-only.

use crate::error::Result;
use refinery::Migration;

refinery::embed_migrations!("src/db/migrations");

/// Apply pending migrations to the writer connection. Forward-only; rollback is not
/// supported and never will be — README §Storage → Engine.
pub fn apply(conn: &mut rusqlite::Connection) -> Result<()> {
    migrations::runner()
        .set_abort_divergent(true)
        .set_abort_missing(true)
        .run(conn)
        .map_err(|e| {
            crate::error::Error::Invariant(string_leak(&format!("migration failed: {e}")))
        })?;
    Ok(())
}

/// Highest applied migration version, or `None` on a fresh DB with no migration history.
pub fn latest_applied(conn: &mut rusqlite::Connection) -> Result<Option<u32>> {
    let last: Option<Migration> = migrations::runner()
        .get_last_applied_migration(conn)
        .map_err(|e| {
            crate::error::Error::Invariant(string_leak(&format!("query last migration: {e}")))
        })?;
    Ok(last.map(|m| m.version()))
}

/// Leak a small diagnostic string so we can pass it through the `&'static str` field
/// in `Error::Invariant`. Used only at startup; not a hot path.
fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
