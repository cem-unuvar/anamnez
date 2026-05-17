//! Argon2id-wrapped recovery path: encrypted SQLCipher passphrase + the physical
//! recovery code printed at first-boot.
//!
//! Envelope layout: `salt (16) || nonce (12) || ciphertext-and-tag`.
//!
//! NOTE: README's plan mentions BIP39 24-word encoding for the recovery code. For
//! Phase 1 this is implemented as 32 random bytes → 64-character hex, to avoid a
//! BIP39 crate dependency. Switch to BIP39 in a later phase before customer ship.

use crate::error::{Error, Result};
use crate::rng::Rng;
use aes_gcm::aead::{Aead, KeyInit};
use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretString};

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// Wrap `passphrase` under a key derived from `recovery_code` via Argon2id.
pub fn wrap(
    passphrase: &SecretString,
    recovery_code: &SecretString,
    rng: &dyn Rng,
) -> Result<Vec<u8>> {
    let mut salt = [0u8; SALT_LEN];
    rng.fill_bytes(&mut salt);
    let key = derive_key(recovery_code.expose_secret().as_bytes(), &salt)?;

    let mut nonce = [0u8; NONCE_LEN];
    rng.fill_bytes(&mut nonce);
    let cipher = aes_gcm::Aes256Gcm::new((&key).into());
    let nonce_obj = aes_gcm::Nonce::from_slice(&nonce);
    let ciphertext = cipher
        .encrypt(nonce_obj, passphrase.expose_secret().as_bytes())
        .map_err(|_| Error::Invariant("recovery wrap encrypt failed"))?;

    let mut envelope = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    envelope.extend_from_slice(&salt);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

/// Unwrap with the same recovery code. Wrong code → tamper-shaped decrypt failure.
pub fn unwrap_with_code(wrapped: &[u8], recovery_code: &SecretString) -> Result<SecretString> {
    if wrapped.len() < SALT_LEN + NONCE_LEN {
        return Err(Error::Invariant("recovery envelope too short"));
    }
    let salt = &wrapped[..SALT_LEN];
    let nonce = &wrapped[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &wrapped[SALT_LEN + NONCE_LEN..];

    let key = derive_key(recovery_code.expose_secret().as_bytes(), salt)?;
    let cipher = aes_gcm::Aes256Gcm::new((&key).into());
    let nonce_obj = aes_gcm::Nonce::from_slice(nonce);
    let plain = cipher
        .decrypt(nonce_obj, ciphertext)
        .map_err(|_| Error::BadCredentials)?;
    let s = String::from_utf8(plain).map_err(|_| Error::Invariant("recovery payload not utf-8"))?;
    Ok(SecretString::from(s))
}

/// Generate a fresh recovery code. Phase 1 form: 64-character hex string of 32 random bytes.
pub fn generate_code(rng: &dyn Rng) -> SecretString {
    let mut buf = [0u8; 32];
    rng.fill_bytes(&mut buf);
    let mut s = String::with_capacity(64);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    SecretString::from(s)
}

fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    let params =
        Params::new(19_456, 2, 1, Some(KEY_LEN)).map_err(|_| Error::Invariant("argon2 params"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut out)
        .map_err(|_| Error::Invariant("argon2id hash_password_into failed"))?;
    Ok(out)
}
