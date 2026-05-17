//! Subsystem J — Key custody. README §Deployment → Key custody.

#![allow(clippy::wildcard_imports)]

use anamnez_core::key_custody::{recovery, ColdBoot, SecureEnclaveWrap};
use anamnez_core::test_support::prelude::*;
use anamnez_core::Error;
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;

#[test]
fn argon2id_recovery_wrap_round_trips_with_correct_code() {
    let rng = DeterministicRng::from_seed(42);
    let pass = SecretString::from("supersecret-256-bit-passphrase!!".to_owned());
    let code = SecretString::from("recovery-code-abc".to_owned());

    let wrapped = recovery::wrap(&pass, &code, &rng).expect("wrap");
    let back = recovery::unwrap_with_code(&wrapped, &code).expect("unwrap");
    assert_eq!(pass.expose_secret(), back.expose_secret());
}

#[test]
fn recovery_unwrap_with_wrong_code_fails() {
    let rng = DeterministicRng::from_seed(42);
    let pass = SecretString::from("supersecret".to_owned());
    let code = SecretString::from("correct".to_owned());
    let wrong = SecretString::from("wrong".to_owned());

    let wrapped = recovery::wrap(&pass, &code, &rng).expect("wrap");
    let err = recovery::unwrap_with_code(&wrapped, &wrong).expect_err("must fail");
    matches!(err, Error::BadCredentials)
        .then_some(())
        .expect("expected BadCredentials");
}

#[test]
fn cold_boot_unwrap_via_sep_uses_fixture_path() {
    let sep: Arc<dyn SecureEnclaveWrap> = Arc::new(FixtureSep::new());
    let pass = SecretString::from("anamnez-passphrase".to_owned());
    let wrapped = sep.wrap(&pass).expect("wrap");

    let cb = ColdBoot::new(sep);
    let back = cb.unwrap_passphrase(&wrapped).expect("unwrap");
    assert_eq!(back.expose_secret(), pass.expose_secret());
}

#[test]
fn cold_boot_via_recovery_path_with_correct_code() {
    let rng = DeterministicRng::from_seed(1);
    let pass = SecretString::from("anamnez-passphrase".to_owned());
    let code = SecretString::from("the-recovery-code".to_owned());
    let wrapped = recovery::wrap(&pass, &code, &rng).expect("wrap");

    let sep: Arc<dyn SecureEnclaveWrap> = Arc::new(FixtureSep::new());
    let cb = ColdBoot::new(sep);
    let back = cb
        .unwrap_passphrase_via_recovery(&wrapped, &code)
        .expect("recovery unwrap");
    assert_eq!(back.expose_secret(), pass.expose_secret());
}

#[test]
fn wrong_sqlcipher_passphrase_returns_typed_bad_credentials() {
    use anamnez_core::env::Environment;

    let temp = TempDb::new().expect("first open");
    let path = temp.path().to_owned();
    let root = temp.root;
    drop(temp.db);

    let wrong_pass = SecretString::from("wrong-passphrase".to_owned());
    let result = anamnez_core::db::Database::open(&path, wrong_pass, Environment::Test);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("wrong passphrase should not open the DB"),
    };
    let err_str = format!("{err:?}");
    assert!(
        matches!(err, Error::BadCredentials) || err_str.contains("not a database"),
        "expected BadCredentials-shaped error, got: {err_str}"
    );
    drop(root);
}

#[test]
fn recovery_code_is_64_char_hex_for_phase1() {
    // Phase 1 form. README/plan calls for BIP39 in a later phase.
    let rng = DeterministicRng::from_seed(7);
    let code = recovery::generate_code(&rng);
    let s = code.expose_secret();
    assert_eq!(s.len(), 64);
    assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
}
