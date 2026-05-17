//! Reusable test assertions.

use crate::audit::Action;
use crate::db::Database;
use crate::error::Result;

/// Assert that an audit row with `action` and `target_id` was appended to the chain.
pub fn assert_audit_appended(_db: &Database, _action: Action, _target_id: &str) -> Result<()> {
    todo!("assertions::assert_audit_appended — Step 1.3")
}

/// Assert that the audit chain verifies cleanly (no tampered rows).
pub fn assert_chain_intact(_db: &Database) -> Result<()> {
    todo!("assertions::assert_chain_intact — Step 1.3")
}
