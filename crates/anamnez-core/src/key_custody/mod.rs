//! README §Deployment → Key custody — SEP wrap + recovery wrap of the SQLCipher passphrase.

pub mod passphrase;
pub mod recovery;
pub mod sep;

use crate::error::Result;
use secrecy::SecretString;

/// Trait for wrapping/unwrapping a passphrase against a hardware-backed key.
pub trait SecureEnclaveWrap: Send + Sync + 'static {
    fn wrap(&self, plaintext: &SecretString) -> Result<Vec<u8>>;
    fn unwrap(&self, wrapped: &[u8]) -> Result<SecretString>;
}

/// Cold-boot orchestrator: try SEP first, fall back to Argon2id-recovery only when
/// the caller explicitly invokes the recovery path.
pub struct ColdBoot {
    sep: std::sync::Arc<dyn SecureEnclaveWrap>,
}

impl ColdBoot {
    pub fn new(sep: std::sync::Arc<dyn SecureEnclaveWrap>) -> Self {
        Self { sep }
    }

    /// Normal cold-boot path. Unwraps the SQLCipher passphrase from a SEP-wrapped
    /// envelope. Returns whatever SEP returns — the daemon panics at startup if this
    /// fails on a healthy appliance.
    pub fn unwrap_passphrase(&self, wrap_sep: &[u8]) -> Result<SecretString> {
        self.sep.unwrap(wrap_sep)
    }

    /// Disaster-recovery path. Used by `anamnez init --restore` after the original
    /// Mac Studio dies and the admin types the printed recovery code into a fresh
    /// machine's wizard.
    pub fn unwrap_passphrase_via_recovery(
        &self,
        wrap_recovery: &[u8],
        recovery_code: &SecretString,
    ) -> Result<SecretString> {
        recovery::unwrap_with_code(wrap_recovery, recovery_code)
    }
}
