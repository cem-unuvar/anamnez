//! Subsystem L — Trait-fixture machinery. README §Testing.

#![allow(clippy::wildcard_imports)]

use anamnez_core::llm::cache_key::CacheKey;
use anamnez_core::test_support::prelude::*;
use jiff::Timestamp;
use std::time::Duration;

#[test]
fn test_clock_starts_at_initial_and_advances() {
    let t0 = Timestamp::from_second(1_700_000_000).unwrap();
    let clock = TestClock::at(t0);
    assert_eq!(anamnez_core::time::Clock::now(&clock), t0);
    clock.advance(Duration::from_secs(60));
    let t1 = anamnez_core::time::Clock::now(&clock);
    assert_eq!(t1.as_second() - t0.as_second(), 60);
}

#[test]
fn test_clock_set_overrides() {
    let t0 = Timestamp::from_second(1_700_000_000).unwrap();
    let t1 = Timestamp::from_second(1_800_000_000).unwrap();
    let clock = TestClock::at(t0);
    clock.set(t1);
    assert_eq!(anamnez_core::time::Clock::now(&clock), t1);
}

#[test]
fn deterministic_rng_reproducible_for_same_seed() {
    use anamnez_core::rng::Rng;
    let r1 = DeterministicRng::from_seed(42);
    let r2 = DeterministicRng::from_seed(42);
    let mut b1 = [0u8; 32];
    let mut b2 = [0u8; 32];
    r1.fill_bytes(&mut b1);
    r2.fill_bytes(&mut b2);
    assert_eq!(b1, b2, "same seed must produce identical bytes");
}

#[test]
fn deterministic_rng_diverges_for_different_seeds() {
    use anamnez_core::rng::Rng;
    let r1 = DeterministicRng::from_seed(1);
    let r2 = DeterministicRng::from_seed(2);
    let mut b1 = [0u8; 16];
    let mut b2 = [0u8; 16];
    r1.fill_bytes(&mut b1);
    r2.fill_bytes(&mut b2);
    assert_ne!(b1, b2);
}

#[test]
fn fixture_sep_round_trips_wrap_unwrap() {
    use anamnez_core::key_custody::SecureEnclaveWrap;
    use secrecy::{ExposeSecret, SecretString};

    let sep = FixtureSep::new();
    let plaintext = SecretString::from("anamnez-passphrase-32-bytes-here".to_owned());
    let wrapped = sep.wrap(&plaintext).expect("wrap");
    let unwrapped = sep.unwrap(&wrapped).expect("unwrap");
    assert_eq!(plaintext.expose_secret(), unwrapped.expose_secret());
}

#[test]
fn cache_key_blake3_hash_matches_canonical_bytes() {
    let key = CacheKey::compose("provider", "model", "hello world", r#"{"temperature":0}"#);
    let canonical = key.canonical_bytes();
    let expected = blake3::hash(&canonical);
    assert_eq!(key.blake3(), *expected.as_bytes());
}

#[test]
fn cache_key_normalizes_whitespace_in_prompt() {
    let a = CacheKey::compose("p", "m", "hello   world", "{}");
    let b = CacheKey::compose("p", "m", "hello world", "{}");
    assert_eq!(a.hex(), b.hex(), "runs of whitespace must collapse");
}

#[test]
#[should_panic(expected = "fixture miss")]
fn fixture_cache_miss_panics_with_record_fixture_instruction() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let cache = FixtureCache::new(temp.path().to_owned(), "llm");
    let _ = cache.get("deadbeef".repeat(8).as_str());
}

#[test]
#[should_panic(expected = "temperature must be 0")]
fn fixture_llm_extractor_panics_on_nonzero_temperature() {
    use anamnez_core::llm::{LlmCall, LlmExtractor};
    let temp = tempfile::TempDir::new().expect("tempdir");
    let extractor = FixtureLlmExtractor::new(FixtureCache::new(temp.path().to_owned(), "llm"));
    let call = LlmCall {
        provider_id: "p".into(),
        model_id: "m".into(),
        system_prompt: "sys".into(),
        user_prompt: "user".into(),
        temperature: 0.5,
    };
    // Drive the future to completion synchronously via a single-thread runtime.
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    let _ = rt.block_on(extractor.complete(call));
}
