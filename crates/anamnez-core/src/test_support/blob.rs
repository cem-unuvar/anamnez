//! Tempdir-backed `FsBlobStore` factory for tests.

use crate::blobs::FsBlobStore;
use crate::error::Result;
use crate::rng::Rng;
use secrecy::SecretString;
use std::sync::Arc;
use tempfile::TempDir;

/// Spin up an `FsBlobStore` rooted at a fresh tempdir.
pub fn fresh(rng: Arc<dyn Rng>, passphrase: SecretString) -> Result<(FsBlobStore, TempDir)> {
    let dir = TempDir::new()?;
    let root = dir.path().to_owned();
    let store = FsBlobStore::new(root, passphrase, rng);
    Ok((store, dir))
}
