//! `SecureEnclaveWrap` fixture. Round-trips wrap/unwrap via XOR with a fixed key —
//! sufficient for tests of the cold-boot orchestrator; production uses the real SEP impl.

use crate::error::{Error, Result};
use crate::key_custody::SecureEnclaveWrap;
use secrecy::{ExposeSecret, SecretString};

const FIXTURE_KEY: &[u8; 32] = b"anamnez-test-sep-fixture-key-32!";

#[derive(Default)]
pub struct FixtureSep {
    _private: (),
}

impl FixtureSep {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecureEnclaveWrap for FixtureSep {
    fn wrap(&self, plaintext: &SecretString) -> Result<Vec<u8>> {
        let bytes = plaintext.expose_secret().as_bytes();
        let mut out = bytes.to_vec();
        xor_in_place(&mut out, FIXTURE_KEY);
        Ok(out)
    }

    fn unwrap(&self, wrapped: &[u8]) -> Result<SecretString> {
        let mut out = wrapped.to_vec();
        xor_in_place(&mut out, FIXTURE_KEY);
        let s = String::from_utf8(out)
            .map_err(|_| Error::Invariant("FixtureSep: unwrapped bytes are not utf-8"))?;
        Ok(SecretString::from(s))
    }
}

fn xor_in_place(buf: &mut [u8], key: &[u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b ^= key[i % key.len()];
    }
}
