//! Content-addressed path layout: `<root>/blobs/<sha[:2]>/<sha>`.

use std::path::{Path, PathBuf};

#[must_use]
pub fn for_sha(root: &Path, sha256_hex: &str) -> PathBuf {
    let prefix = &sha256_hex[..2.min(sha256_hex.len())];
    root.join("blobs").join(prefix).join(sha256_hex)
}
