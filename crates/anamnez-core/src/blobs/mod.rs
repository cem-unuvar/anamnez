//! README §Storage → Source documents — content-addressed blob store, AES-GCM per file.

pub mod encryption;
pub mod path;
pub mod verify;

use crate::error::{Error, Result};
use crate::rng::Rng;
use secrecy::{ExposeSecret, SecretString};
use std::path::PathBuf;
use std::sync::Arc;

pub trait BlobStore: Send + Sync + 'static {
    fn store(&self, bytes: &[u8]) -> Result<String>;
    fn get(&self, sha256: &str) -> Result<Vec<u8>>;
    fn exists(&self, sha256: &str) -> Result<bool>;
}

/// Filesystem-backed implementation rooted at `root`. Files are AES-GCM-encrypted
/// with a key derived from the SQLCipher passphrase.
pub struct FsBlobStore {
    root: PathBuf,
    key: [u8; 32],
    rng: Arc<dyn Rng>,
}

impl FsBlobStore {
    pub fn new(root: PathBuf, passphrase: SecretString, rng: Arc<dyn Rng>) -> Self {
        let key = encryption::derive_key_from_passphrase(passphrase.expose_secret().as_bytes());
        Self { root, key, rng }
    }
}

impl BlobStore for FsBlobStore {
    fn store(&self, bytes: &[u8]) -> Result<String> {
        let sha = verify::sha256_hex(bytes);
        let envelope = encryption::encrypt(&self.key, bytes, &*self.rng)?;
        let path = path::for_sha(&self.root, &sha);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &envelope)?;
        Ok(sha)
    }

    fn get(&self, sha256: &str) -> Result<Vec<u8>> {
        let path = path::for_sha(&self.root, sha256);
        let envelope = std::fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotFound
            } else {
                Error::Io(e)
            }
        })?;
        let plaintext = encryption::decrypt(&self.key, &envelope)?;
        // Panic-loud on sha256 mismatch — data corruption is non-recoverable.
        verify::assert_sha_matches(&plaintext, sha256);
        Ok(plaintext)
    }

    fn exists(&self, sha256: &str) -> Result<bool> {
        let path = path::for_sha(&self.root, sha256);
        Ok(path.exists())
    }
}
