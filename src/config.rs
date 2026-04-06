#![allow(dead_code)] // Stub: implemented in Phase 9

use serde::Deserialize;

/// Top-level configuration loaded from `~/.config/sift/config.toml`.
///
/// All fields are optional in the TOML file — missing fields use their
/// `Default` implementation. A missing config file is not an error.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub defaults: DefaultsConfig,
    pub tracking: TrackingConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    pub verbosity: String,
    pub max_lines: usize,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TrackingConfig {
    pub enabled: bool,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            verbosity: "compact".to_string(),
            max_lines: 100,
        }
    }
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Load configuration from `~/.config/sift/config.toml`.
///
/// Returns `Config::default()` if the file does not exist.
/// Returns an error only if the file exists but cannot be parsed.
pub fn load() -> Config {
    // Phase 9: real implementation
    Config::default()
}
