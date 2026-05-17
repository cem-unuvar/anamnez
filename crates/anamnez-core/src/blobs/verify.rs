//! Read-time sha256 verification. README: panic on mismatch.

use sha2::{Digest, Sha256};

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out.iter() {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Verify that `bytes`' SHA-256 hex equals `expected_hex`. Panics on mismatch —
/// per README §Storage → Source documents, this is a data-corruption signal we
/// want loud, not silently recoverable.
pub fn assert_sha_matches(bytes: &[u8], expected_hex: &str) {
    let actual = sha256_hex(bytes);
    assert_eq!(
        actual, expected_hex,
        "blob sha256 mismatch — file on disk has been corrupted or tampered with"
    );
}
