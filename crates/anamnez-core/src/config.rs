//! Daemon configuration — loud validation per README §Development.

use crate::auth::client_version::Version;
use crate::env::Environment;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub environment: Environment,
    pub db_path: PathBuf,
    pub blob_root: PathBuf,
    pub idle_lock_minutes: u32,
    pub min_client_version: Version,
    pub code_systems_root: PathBuf,
}

impl Config {
    /// Validate the config — fails loud on any invalid combination.
    pub fn validate(&self) -> Result<()> {
        if !self.db_path.parent().is_some_and(std::path::Path::is_dir) {
            return Err(crate::error::Error::Invariant(
                "config: db_path parent directory does not exist",
            ));
        }
        if !self.blob_root.is_dir() {
            return Err(crate::error::Error::Invariant(
                "config: blob_root is not a directory",
            ));
        }
        if !(5..=30).contains(&self.idle_lock_minutes) {
            return Err(crate::error::Error::Invariant(
                "config: idle_lock_minutes must be in [5, 30]",
            ));
        }
        if !self.code_systems_root.is_dir() {
            return Err(crate::error::Error::Invariant(
                "config: code_systems_root is not a directory",
            ));
        }
        Ok(())
    }
}
