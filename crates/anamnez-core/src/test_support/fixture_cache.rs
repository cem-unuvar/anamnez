//! On-disk fixture cache for LLM / OCR / STT calls.
//!
//! Layout: `<root>/<provider>/<hash>.json`. Cache miss panics with the
//! `cargo xtask record-fixture` instruction — the only path in the repo that talks
//! to a real model.

use crate::error::{Error, Result};
use std::path::PathBuf;

pub struct FixtureCache {
    pub root: PathBuf,
    pub provider: String,
}

impl FixtureCache {
    #[must_use]
    pub fn new(root: PathBuf, provider: impl Into<String>) -> Self {
        Self {
            root,
            provider: provider.into(),
        }
    }

    /// Resolve the on-disk path for a fixture key.
    #[must_use]
    pub fn path_for(&self, hash_hex: &str) -> PathBuf {
        self.root
            .join(&self.provider)
            .join(format!("{hash_hex}.json"))
    }

    /// Look up a fixture by its hex cache key.
    ///
    /// Miss is a hard test failure with an instruction to record the fixture. Never
    /// returns a default — README §Testing: "A miss is a hard test failure with a
    /// message instructing the developer to run `cargo xtask record-fixture <key>`."
    pub fn get(&self, hash_hex: &str) -> Result<serde_json::Value> {
        let path = self.path_for(hash_hex);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                panic!(
                    "fixture miss: {provider}/{hash}.json — run `cargo xtask record-fixture {provider} {hash}` (expected at {path})",
                    provider = self.provider,
                    hash = hash_hex,
                    path = path.display(),
                );
            }
        };
        let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            Error::Invariant(string_leak(&format!("fixture {hash_hex} not JSON: {e}")))
        })?;
        Ok(v)
    }
}

fn string_leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
