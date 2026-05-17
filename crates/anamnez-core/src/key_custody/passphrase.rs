//! Typed SQLCipher passphrase wrapper. Generation, wrapping, and verification.

use crate::rng::Rng;
use secrecy::SecretString;
use std::fmt::Write;

/// Length of the random SQLCipher passphrase in bytes.
pub const PASSPHRASE_LEN: usize = 32;

/// Generate a fresh random 256-bit SQLCipher passphrase. Never displayed.
pub fn generate(rng: &dyn Rng) -> SecretString {
    let mut buf = [0u8; PASSPHRASE_LEN];
    rng.fill_bytes(&mut buf);
    let mut s = String::with_capacity(buf.len() * 2);
    for b in buf {
        let _ = write!(s, "{b:02x}");
    }
    SecretString::from(s)
}
