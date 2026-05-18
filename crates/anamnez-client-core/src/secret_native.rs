//! Native `SecretStore` backed by the OS keychain (`keyring` crate covers both macOS
//! Keychain and Windows Credential Manager). The slots are:
//!
//! | slot                  | what it holds                                |
//! |-----------------------|----------------------------------------------|
//! | `device_cert_pem`     | mTLS client cert PEM, issued by daemon CA    |
//! | `device_key_pem`      | mTLS client private key PEM                  |
//! | `ca_cert_pem`         | daemon's CA cert PEM                         |
//! | `refresh_token`       | rotating refresh token, one-time-use         |
//!
//! All four are written once at successful enrollment (`device_*` + `ca_cert_pem`) and
//! at successful login (`refresh_token`). `refresh_token` is rewritten on every refresh.

use keyring::Entry;

use crate::error::ClientError;

const SERVICE: &str = "org.anamnez.workstation";

#[derive(Debug, Clone, Copy)]
pub enum Slot {
    DeviceCertPem,
    DeviceKeyPem,
    CaCertPem,
    RefreshToken,
}

impl Slot {
    fn key(self) -> &'static str {
        match self {
            Slot::DeviceCertPem => "device_cert_pem",
            Slot::DeviceKeyPem => "device_key_pem",
            Slot::CaCertPem => "ca_cert_pem",
            Slot::RefreshToken => "refresh_token",
        }
    }
}

fn entry(slot: Slot) -> Result<Entry, ClientError> {
    Entry::new(SERVICE, slot.key()).map_err(|e| ClientError::Transport(format!("keyring: {e}")))
}

pub fn get(slot: Slot) -> Result<Option<String>, ClientError> {
    let e = entry(slot)?;
    match e.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(other) => Err(ClientError::Transport(format!("keyring get: {other}"))),
    }
}

pub fn put(slot: Slot, value: &str) -> Result<(), ClientError> {
    let e = entry(slot)?;
    e.set_password(value)
        .map_err(|e| ClientError::Transport(format!("keyring put: {e}")))
}

pub fn delete(slot: Slot) -> Result<(), ClientError> {
    let e = entry(slot)?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(other) => Err(ClientError::Transport(format!("keyring delete: {other}"))),
    }
}
