//! Argon2id password hashing. README §Tenancy.

use crate::error::{Error, Result};
use argon2::password_hash::{
    rand_core::OsRng as ArgonOsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use secrecy::{ExposeSecret, SecretString};

/// Hash a password with Argon2id and OWASP-2024 parameters.
pub fn hash(password: SecretString) -> Result<String> {
    let salt = SaltString::generate(&mut ArgonOsRng);
    let argon2 = Argon2::default();
    let s = argon2
        .hash_password(password.expose_secret().as_bytes(), &salt)
        .map_err(|_| Error::Invariant("argon2id hash failed"))?
        .to_string();
    Ok(s)
}

/// Verify a password against a stored hash. Constant-time.
pub fn verify(password: SecretString, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|_| Error::Invariant("argon2 hash parse"))?;
    let argon2 = Argon2::default();
    Ok(argon2
        .verify_password(password.expose_secret().as_bytes(), &parsed)
        .is_ok())
}
