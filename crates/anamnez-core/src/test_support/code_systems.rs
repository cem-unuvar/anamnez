//! Cached code-systems template DB. `OnceLock<Vec<u8>>` holds a serialized template;
//! per-test `TempDb::new` deep-copies the template into a fresh tempdir.

use crate::error::Result;
use std::path::Path;

/// Build (or reuse from cache) a template DB with code-systems loaded. Returns
/// the bytes of the template SQLite file.
pub fn template_bytes(_code_systems_root: &Path) -> Result<&'static [u8]> {
    todo!("test_support::code_systems::template_bytes — Step 1.3")
}
