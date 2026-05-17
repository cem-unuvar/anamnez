//! Opaque bearer tokens (15-min access, 12-hr refresh).
//!
//! Stored at rest as SHA-256 hashes; the unhashed forms exist only in the response body
//! and in the workstation's OS secret store.

use crate::error::Result;
use crate::rng::Rng;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const ACCESS_TOKEN_MINUTES: i64 = 15;
pub const REFRESH_TOKEN_HOURS: i64 = 12;

pub const TOKEN_LEN_BYTES: usize = 32;

/// Generate a fresh opaque token (256 bits of entropy, hex-encoded).
pub fn fresh(rng: &dyn Rng) -> SecretString {
    let mut buf = [0u8; TOKEN_LEN_BYTES];
    rng.fill_bytes(&mut buf);
    let mut s = String::with_capacity(buf.len() * 2);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    SecretString::from(s)
}

/// SHA-256 a token for at-rest storage.
#[must_use]
pub fn hash(token: &SecretString) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(token.expose_secret().as_bytes());
    h.finalize().to_vec()
}

/// Constant-time compare a token against a stored hash.
#[must_use]
pub fn matches(token: &SecretString, stored_hash: &[u8]) -> bool {
    let computed = hash(token);
    computed.ct_eq(stored_hash).into()
}

/// Whether an access token is structurally well-formed. Not a credential check.
pub fn looks_valid(token: &SecretString) -> Result<()> {
    let s = token.expose_secret();
    if s.len() == TOKEN_LEN_BYTES * 2 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(crate::error::Error::BadCredentials)
    }
}
