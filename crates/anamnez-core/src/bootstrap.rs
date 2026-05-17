//! SPEC §Deployment — first-boot bootstrap that lays down the on-disk artifacts
//! required for `anamnez serve` to come up.
//!
//! Idempotent: errors if `data_dir/anamnez.sqlite` already exists. The future
//! `anamnez init` subcommand and layer-2 tests both call this entry point — `init`
//! drives it interactively with prompts; tests pass canned credentials directly.
//!
//! On-disk layout produced by `run`:
//!   data_dir/
//!     anamnez.sqlite           # SQLCipher DB with first migration + env marker + admin row
//!     wrap_sep.bin             # SEP-wrapped SQLCipher passphrase (production boot path)
//!     wrap_recovery.bin        # Argon2id-wrapped SQLCipher passphrase (recovery path)
//!     tls/
//!       server_cert.pem
//!       server_key.pem
//!       ca_cert.pem
//!
//! The TLS PEMs are inputs, not outputs: callers mint them with their preferred
//! mechanism (`rcgen` in the daemon's bootstrap helper, real first-boot wizard
//! later). Keeping cert minting out of core keeps the wasm-irrelevant cert crates
//! off the core dep tree.

use crate::audit::{self, Action, AppendInput};
use crate::auth::password;
use crate::db::Database;
use crate::env::Environment;
use crate::error::{Error, Result};
use crate::ids::UserId;
use crate::key_custody::{passphrase, recovery, SecureEnclaveWrap};
use crate::rng::Rng;
use secrecy::SecretString;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

pub struct BootstrapInputs<'a> {
    pub data_dir: &'a Path,
    pub environment: Environment,
    pub admin_email: String,
    pub admin_password: SecretString,
    pub admin_display_name: String,
    pub sep: Arc<dyn SecureEnclaveWrap>,
    pub rng: Arc<dyn Rng>,
    pub server_cert_pem: String,
    pub server_key_pem: String,
    pub ca_cert_pem: String,
}

pub struct BootstrapArtifacts {
    pub admin_user_id: UserId,
    /// The recovery code, printed once at first boot. Caller persists it to
    /// physical paper; the daemon never stores it.
    pub recovery_code: SecretString,
}

/// Produce the on-disk artifacts described in the module docs. Idempotent on the
/// absence of `anamnez.sqlite`; otherwise errors `Error::Invariant("already initialized")`.
pub fn run(inputs: BootstrapInputs<'_>) -> Result<BootstrapArtifacts> {
    let db_path = inputs.data_dir.join("anamnez.sqlite");
    if db_path.exists() {
        return Err(Error::Invariant("bootstrap: already initialized"));
    }
    std::fs::create_dir_all(inputs.data_dir)?;
    std::fs::create_dir_all(inputs.data_dir.join("tls"))?;

    // Mint passphrase + recovery code.
    let pass = passphrase::generate(&*inputs.rng);
    let code = recovery::generate_code(&*inputs.rng);

    // Wrap both ways and persist.
    let wrap_sep = inputs.sep.wrap(&pass)?;
    let wrap_recovery = recovery::wrap(&pass, &code, &*inputs.rng)?;
    std::fs::write(inputs.data_dir.join("wrap_sep.bin"), &wrap_sep)?;
    std::fs::write(inputs.data_dir.join("wrap_recovery.bin"), &wrap_recovery)?;

    // TLS PEMs the caller minted.
    std::fs::write(
        inputs.data_dir.join("tls").join("server_cert.pem"),
        &inputs.server_cert_pem,
    )?;
    std::fs::write(
        inputs.data_dir.join("tls").join("server_key.pem"),
        &inputs.server_key_pem,
    )?;
    std::fs::write(
        inputs.data_dir.join("tls").join("ca_cert.pem"),
        &inputs.ca_cert_pem,
    )?;

    // Open DB (runs migrations + env marker on first boot).
    let db = Database::open(&db_path, pass, inputs.environment)?;

    // Insert the admin row.
    let admin_id = UserId::new();
    let admin_hash = password::hash(inputs.admin_password)?;
    let now = db.clock().now();
    db.with_writer(|conn| {
        conn.execute(
            "INSERT INTO user (id, email, display_name, role, password_hash, created_at) \
             VALUES (?1, ?2, ?3, 'admin', ?4, ?5)",
            rusqlite::params![
                admin_id.as_uuid().to_string(),
                inputs.admin_email,
                inputs.admin_display_name,
                admin_hash,
                now.to_string(),
            ],
        )?;
        audit::append_in_conn(
            conn,
            now,
            AppendInput {
                actor_user_id: Some(admin_id),
                auth_session_id: None,
                action: Action::UserCreate,
                target_type: "user".into(),
                target_id: admin_id.as_uuid().to_string(),
                patient_id: None,
                metadata: json!({"email": inputs.admin_email, "role": "admin", "via": "bootstrap"}),
            },
        )?;
        Ok(())
    })?;

    Ok(BootstrapArtifacts {
        admin_user_id: admin_id,
        recovery_code: code,
    })
}
