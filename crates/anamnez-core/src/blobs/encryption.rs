//! AES-GCM per-file with a fresh random nonce.

use crate::error::{Error, Result};
use crate::rng::Rng;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;

/// Encrypt `plaintext`. Envelope: `nonce || ciphertext-and-tag`.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8], rng: &dyn Rng) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| Error::Invariant("AES-GCM encrypt failed"))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt the `nonce || ciphertext-and-tag` envelope. Tamper → `Error::Invariant`.
pub fn decrypt(key: &[u8; 32], envelope: &[u8]) -> Result<Vec<u8>> {
    if envelope.len() < NONCE_LEN + TAG_LEN {
        return Err(Error::Invariant("AES-GCM envelope too short"));
    }
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(&envelope[..NONCE_LEN]);
    cipher
        .decrypt(nonce, &envelope[NONCE_LEN..])
        .map_err(|_| Error::Invariant("AES-GCM decrypt failed (tampered ciphertext)"))
}

/// Derive a 32-byte blob-encryption key from the SQLCipher passphrase root.
#[must_use]
pub fn derive_key_from_passphrase(passphrase: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(passphrase);
    h.update(b"\x1fanamnez-blob-key-v1");
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}
