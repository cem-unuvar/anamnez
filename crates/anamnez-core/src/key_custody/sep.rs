//! macOS Secure Enclave wrap (production). Stub for Phase 1 — real impl in Phase 6.

use super::SecureEnclaveWrap;
use crate::error::{Error, Result};
use secrecy::SecretString;

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
