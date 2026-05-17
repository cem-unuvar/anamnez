//! TOML config loader. The config schema lives in `anamnez_core::config::Config`.

use anamnez_core::config::Config;
use anamnez_core::error::{Error, Result};
use std::path::Path;

pub fn load(path: &Path) -> Result<Config> {
    let bytes = std::fs::read_to_string(path).map_err(Error::from)?;
    let cfg: Config = toml::from_str(&bytes).map_err(|e| {
        // Leak the error string so it can be returned as `Error::Invariant(&'static str)`.
        // Acceptable for a startup-time loud failure that crashes the daemon.
        let leaked: &'static str = Box::leak(format!("config parse: {e}").into_boxed_str());
        Error::Invariant(leaked)
    })?;
    Ok(cfg)
}
