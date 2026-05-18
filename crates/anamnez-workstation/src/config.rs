//! Workstation config — on-disk TOML next to OS-conventional app paths. No PHI,
//! no secrets. Contains the daemon's host + pinned fingerprint + last known
//! `idle_lock_minutes` policy + a couple of UI prefs.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("no platform config dir for this OS")]
    NoConfigDir,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml: {0}")]
    Toml(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daemon: Option<DaemonConfig>,
    #[serde(default = "default_idle_lock_minutes")]
    pub idle_lock_minutes_cache: u32,
}

fn default_idle_lock_minutes() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub base_url: String,
    pub server_fingerprint_sha256: String,
    pub workstation_id: String,
}

pub fn config_dir() -> Result<PathBuf, ConfigError> {
    ProjectDirs::from("org", "anamnez", "workstation")
        .map(|p| p.config_dir().to_path_buf())
        .ok_or(ConfigError::NoConfigDir)
}

pub fn config_path() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn load() -> Result<Config, ConfigError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    toml::from_str(&raw).map_err(|e| ConfigError::Toml(e.to_string()))
}

pub fn save(cfg: &Config) -> Result<(), ConfigError> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("config.toml");
    let s = toml::to_string_pretty(cfg).map_err(|e| ConfigError::Toml(e.to_string()))?;
    write_atomic(&path, &s)?;
    Ok(())
}

fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut tmp = path.to_path_buf();
    tmp.set_extension("toml.new");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
