//! Shared SQLCipher passphrase unwrap used by every subcommand that opens the
//! DB. Hoisted out of `serve/boot.rs` so the CLI subcommands and `serve` share
//! exactly the same conventions.
//!
//! Order of preference:
//! 1. `ANAMNEZ_RECOVERY_CODE` env var → Argon2id-unwrap `wrap_recovery.bin`.
//! 2. Otherwise → `DevSep`-unwrap `wrap_sep.bin` (Phase-1 placeholder for the
//!    Mac Studio Secure Enclave; replaced wholesale when the real SEP
//!    integration lands).

use anamnez_core::error::{Error, Result};
use anamnez_core::key_custody::sep::DevSep;
use anamnez_core::key_custody::{ColdBoot, SecureEnclaveWrap};
use secrecy::SecretString;
use std::path::Path;
use std::sync::Arc;

pub const ENV_RECOVERY_CODE: &str = "ANAMNEZ_RECOVERY_CODE";

/// Return the SQLCipher passphrase for the appliance state under `data_dir`.
pub fn unwrap_for(data_dir: &Path) -> Result<SecretString> {
    if let Ok(code) = std::env::var(ENV_RECOVERY_CODE) {
        let bytes = std::fs::read(data_dir.join("wrap_recovery.bin"))?;
        let cb = ColdBoot::new(Arc::new(NoSep));
        return cb.unwrap_passphrase_via_recovery(&bytes, &SecretString::from(code));
    }
    let sep: Arc<dyn SecureEnclaveWrap> = Arc::new(DevSep::new());
    let cb = ColdBoot::new(sep);
    let bytes = std::fs::read(data_dir.join("wrap_sep.bin"))?;
    cb.unwrap_passphrase(&bytes)
}

/// The SEP impl used at `init` time for `wrap_sep.bin` minting. Same Phase-1
/// placeholder as the unwrap side — round-trips deterministically.
#[must_use]
pub fn default_sep() -> Arc<dyn SecureEnclaveWrap> {
    Arc::new(DevSep::new())
}

/// `ColdBoot::unwrap_passphrase_via_recovery` requires a `SecureEnclaveWrap` at
/// construction time even though the recovery path doesn't touch SEP. We pass
/// a tombstone that errors loudly if accidentally invoked.
struct NoSep;
impl SecureEnclaveWrap for NoSep {
    fn wrap(&self, _: &SecretString) -> Result<Vec<u8>> {
        Err(Error::Invariant("SEP unused on recovery path"))
    }
    fn unwrap(&self, _: &[u8]) -> Result<SecretString> {
        Err(Error::Invariant("SEP unused on recovery path"))
    }
}
