//! Subsystem H — Blob store. README §Storage → Source documents.

#![allow(clippy::wildcard_imports)]

use anamnez_core::blobs::{path, verify, BlobStore};
use anamnez_core::test_support::prelude::*;
use secrecy::SecretString;
use std::sync::Arc;

#[test]
fn content_addressed_path_uses_two_char_prefix_directory() {
    let p = path::for_sha(std::path::Path::new("/var/data"), "deadbeef0011");
    let s = p.to_string_lossy();
    assert!(s.contains("blobs/de/deadbeef0011"), "got {s}");
}

#[test]
fn store_and_get_round_trip_via_sha256() {
    let rng: Arc<dyn anamnez_core::rng::Rng> = Arc::new(DeterministicRng::from_seed(7));
    let pass = SecretString::from("anamnez-test-passphrase".to_owned());
    let (store, _dir) = anamnez_core::test_support::blob::fresh(rng, pass).expect("blob store");

    let payload = b"hello world".to_vec();
    let sha = store.store(&payload).expect("store");
    assert_eq!(
        sha,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
    let got = store.get(&sha).expect("get");
    assert_eq!(got, payload);
}

#[test]
fn aes_gcm_tampered_ciphertext_fails_decrypt() {
    use anamnez_core::blobs::encryption;

    let rng = DeterministicRng::from_seed(42);
    let key = [7u8; 32];
    let envelope = encryption::encrypt(&key, b"sensitive", &rng).expect("encrypt");

    let mut tampered = envelope.clone();
    let i = tampered.len() / 2;
    tampered[i] ^= 0xff;

    let err = encryption::decrypt(&key, &tampered).expect_err("tamper must fail");
    assert!(
        format!("{err}").to_ascii_lowercase().contains("tamper"),
        "got: {err}"
    );
}

#[test]
#[should_panic(expected = "blob sha256 mismatch")]
fn read_recomputes_sha256_and_panics_on_mismatch() {
    verify::assert_sha_matches(b"hello", "deadbeef");
}

#[test]
fn nonce_is_random_per_file_so_two_encryptions_of_same_plaintext_differ() {
    use anamnez_core::blobs::encryption;

    let rng = DeterministicRng::from_seed(1);
    let key = [3u8; 32];
    let a = encryption::encrypt(&key, b"identical", &rng).expect("encrypt 1");
    let b = encryption::encrypt(&key, b"identical", &rng).expect("encrypt 2");
    assert_ne!(
        a, b,
        "fresh nonce must make identical plaintexts produce different envelopes"
    );
    assert_eq!(
        encryption::decrypt(&key, &a).expect("dec a"),
        encryption::decrypt(&key, &b).expect("dec b"),
        "both still decrypt to the same plaintext"
    );
}
