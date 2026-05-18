//! Resolved dev-time paths. Everything lives under `target/dev-data/` so a
//! `cargo clean` resets it.

use std::path::{Path, PathBuf};

#[must_use]
pub fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for xtask is `<workspace>/crates/xtask`. Two `parent`s up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

#[must_use]
pub fn data_dir() -> PathBuf {
    if let Ok(env) = std::env::var("ANAMNEZ_DEV_DATA_DIR") {
        return PathBuf::from(env);
    }
    workspace_root().join("target").join("dev-data")
}

#[must_use]
pub fn config_toml() -> PathBuf {
    data_dir().join("config.toml")
}

#[must_use]
pub fn pid_file() -> PathBuf {
    data_dir().join("anamnez.pid")
}

#[must_use]
pub fn recovery_code_file() -> PathBuf {
    data_dir().join(".recovery-code")
}

#[must_use]
pub fn last_enrollment_uri_file() -> PathBuf {
    data_dir().join(".last-enrollment-uri")
}

#[must_use]
pub fn dev_workstation_dir() -> PathBuf {
    data_dir().join("dev-workstation")
}

#[must_use]
pub fn dev_workstation_cert() -> PathBuf {
    dev_workstation_dir().join("cert.pem")
}

#[must_use]
pub fn dev_workstation_key() -> PathBuf {
    dev_workstation_dir().join("key.pem")
}

pub const BIND: &str = "127.0.0.1:8443";
pub const ADMIN_EMAIL: &str = "dev@anamnez.local";
pub const ADMIN_PASSWORD: &str = "[TEST]-dev-password";
pub const ADMIN_DISPLAY_NAME: &str = "Dev Admin";
