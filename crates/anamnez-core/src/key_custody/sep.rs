//! macOS Secure Enclave wrap (production). Stub for Phase 1 — real impl in Phase 6.
//!
//! `DevSep` is the Phase-1 placeholder shipped in production binaries until the
//! real Secure Enclave integration lands. It is XOR with a hardcoded key — not a
//! security boundary. The recovery-code path is the only credible Phase-1 secret.

use super::SecureEnclaveWrap;
use crate::error::{Error, Result};
use secrecy::{ExposeSecret, SecretString};

pub struct MacosSep {
    _private: (),
}

impl MacosSep {
    pub fn new() -> Result<Self> {
        Err(Error::Invariant(
            "MacosSep::new — Phase 1 stub, real impl lands in Phase 6",
        ))
    }
}

impl SecureEnclaveWrap for MacosSep {
    fn wrap(&self, _plaintext: &SecretString) -> Result<Vec<u8>> {
        Err(Error::Invariant("MacosSep::wrap not available in Phase 1"))
    }

    fn unwrap(&self, _wrapped: &[u8]) -> Result<SecretString> {
        Err(Error::Invariant(
            "MacosSep::unwrap not available in Phase 1",
        ))
    }
}

/// Phase-1 development placeholder for the SEP wrap. **Not a security boundary**:
/// every binary holds the same key, so anyone with `wrap_sep.bin` can unwrap it.
/// The recovery-code path is the actual Phase-1 secret. Replaced wholesale when
/// the real Mac Studio Secure Enclave integration lands.
#[derive(Default)]
pub struct DevSep {
    _private: (),
}

const DEV_SEP_KEY: &[u8; 32] = b"anamnez-dev-sep-phase1-rotate-me";

impl DevSep {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecureEnclaveWrap for DevSep {
    fn wrap(&self, plaintext: &SecretString) -> Result<Vec<u8>> {
        let mut buf = plaintext.expose_secret().as_bytes().to_vec();
        xor_in_place(&mut buf, DEV_SEP_KEY);
        Ok(buf)
    }

    fn unwrap(&self, wrapped: &[u8]) -> Result<SecretString> {
        let mut buf = wrapped.to_vec();
        xor_in_place(&mut buf, DEV_SEP_KEY);
        let s = String::from_utf8(buf)
            .map_err(|_| Error::Invariant("DevSep: unwrapped bytes are not utf-8"))?;
        Ok(SecretString::from(s))
    }
}

fn xor_in_place(buf: &mut [u8], key: &[u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b ^= key[i % key.len()];
    }
}
