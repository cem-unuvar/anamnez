//! Cache key for LLM / OCR / STT calls.
//!
//! Composition: `BLAKE3(provider_id || 0x1f || model_id || 0x1f ||
//!                      normalize_prompt(prompt) || 0x1f || canonical_json(params))`.
//!
//! `normalize_prompt`: NFC + collapse runs of whitespace to a single space.

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone)]
pub struct CacheKey {
    pub provider_id: String,
    pub model_id: String,
    pub normalized_prompt: String,
    pub params_canonical: String,
}

impl CacheKey {
    /// Compose a `CacheKey` from raw inputs.
    #[must_use]
    pub fn compose(provider_id: &str, model_id: &str, prompt: &str, params_json: &str) -> Self {
        Self {
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
            normalized_prompt: normalize_prompt(prompt),
            params_canonical: params_json.to_owned(),
        }
    }

    /// Bytes that get hashed.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            self.provider_id.len()
                + self.model_id.len()
                + self.normalized_prompt.len()
                + self.params_canonical.len()
                + 3,
        );
        out.extend_from_slice(self.provider_id.as_bytes());
        out.push(0x1f);
        out.extend_from_slice(self.model_id.as_bytes());
        out.push(0x1f);
        out.extend_from_slice(self.normalized_prompt.as_bytes());
        out.push(0x1f);
        out.extend_from_slice(self.params_canonical.as_bytes());
        out
    }

    /// 32-byte BLAKE3 hash of the canonical key bytes.
    #[must_use]
    pub fn blake3(&self) -> [u8; 32] {
        let hash = blake3::hash(&self.canonical_bytes());
        *hash.as_bytes()
    }

    /// Hex-encoded blake3 for filenames.
    #[must_use]
    pub fn hex(&self) -> String {
        let bytes = self.blake3();
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
        }
        s
    }
}

fn normalize_prompt(prompt: &str) -> String {
    // NFC normalize, then collapse runs of whitespace.
    let nfc: String = prompt.nfc().collect();
    let mut out = String::with_capacity(nfc.len());
    let mut last_was_ws = false;
    for ch in nfc.chars() {
        if ch.is_whitespace() {
            if !last_was_ws {
                out.push(' ');
                last_was_ws = true;
            }
        } else {
            out.push(ch);
            last_was_ws = false;
        }
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize_prompt("  hello   world  "), "hello world");
        assert_eq!(normalize_prompt("foo\nbar\t\tbaz"), "foo bar baz");
    }

    #[test]
    fn cache_key_is_deterministic() {
        let a = CacheKey::compose("p", "m", "prompt", r#"{"t":0}"#);
        let b = CacheKey::compose("p", "m", "prompt", r#"{"t":0}"#);
        assert_eq!(a.hex(), b.hex());
    }

    #[test]
    fn cache_key_changes_with_inputs() {
        let a = CacheKey::compose("p1", "m", "prompt", "{}");
        let b = CacheKey::compose("p2", "m", "prompt", "{}");
        assert_ne!(a.hex(), b.hex());
    }
}
