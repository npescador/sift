use serde::Deserialize;
use std::collections::HashMap;

/// Top-level configuration loaded from `~/.config/sift/config.toml`.
///
/// All fields are optional in the TOML file — missing fields use their
/// `Default` implementation. A missing config file is not an error.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub defaults: DefaultsConfig,
    pub tracking: TrackingConfig,
    pub tee: TeeConfig,
    pub streaming: StreamingConfig,
    /// Per-command overrides. Keys are command family names (e.g. `"git"`,
    /// `"xcodebuild"`). Example config:
    ///
    /// ```toml
    /// [commands.git]
    /// verbosity = "verbose"
    ///
    /// [commands.xcodebuild]
    /// verbosity = "compact"
    /// max_lines = 30
    /// ```
    pub commands: HashMap<String, CommandConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    pub verbosity: String,
    /// Phase 10: wired into per-filter max_lines cap.
    #[allow(dead_code)]
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

/// Configuration for tee mode — saving raw output when a filter produces nothing.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TeeConfig {
    /// When true, save raw output to disk if the filter produces empty content.
    pub enabled: bool,
}

impl Default for TeeConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Configuration for streaming mode — progressive output for long-running commands.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StreamingConfig {
    /// When true, enable streaming by default (can be overridden with `--stream`).
    pub enabled: bool,
}

/// Per-command configuration override.
///
/// All fields are optional — only set the ones you want to override.
/// Unset fields inherit from `[defaults]`.
///
/// Valid `verbosity` values: `"compact"`, `"verbose"`, `"very_verbose"`,
/// `"maximum"`, `"raw"`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CommandConfig {
    /// Override verbosity for this command. Empty string means "use default".
    pub verbosity: String,
    /// Override maximum output lines for this command. `0` means "use default".
    pub max_lines: usize,
}

impl Config {
    /// Resolve the effective verbosity for a command invocation.
    ///
    /// Priority (highest to lowest):
    /// 1. CLI flags (`--raw`, `-v`, `-vv`, `-vvv`) — always wins
    /// 2. Per-command `[commands.<name>] verbosity` override
    /// 3. Global `[defaults] verbosity`
    /// 4. Built-in default (`compact`)
    ///
    /// `cli_override` is `Some(verbosity)` when the user passed an explicit
    /// CLI flag, and `None` when no flag was given.
    pub fn resolve_verbosity(
        &self,
        family_name: &str,
        cli_override: Option<crate::filters::Verbosity>,
    ) -> crate::filters::Verbosity {
        // CLI flag always wins
        if let Some(v) = cli_override {
            return v;
        }

        // Per-command override
        if let Some(cmd_cfg) = self.commands.get(family_name) {
            if !cmd_cfg.verbosity.is_empty() {
                return parse_verbosity(&cmd_cfg.verbosity);
            }
        }

        // Global default
        parse_verbosity(&self.defaults.verbosity)
    }
}

/// Parse a verbosity string from config into a [`crate::filters::Verbosity`] level.
///
/// Unknown values fall back to `Compact` — never error on bad config.
pub fn parse_verbosity(s: &str) -> crate::filters::Verbosity {
    match s {
        "verbose" => crate::filters::Verbosity::Verbose,
        "very_verbose" | "very-verbose" => crate::filters::Verbosity::VeryVerbose,
        "maximum" => crate::filters::Verbosity::Maximum,
        "raw" => crate::filters::Verbosity::Raw,
        _ => crate::filters::Verbosity::Compact,
    }
}

/// Load configuration from `~/.config/sift/config.toml`.
///
/// Resolution order:
/// 1. `$XDG_CONFIG_HOME/sift/config.toml` if `XDG_CONFIG_HOME` is set
/// 2. `$HOME/.config/sift/config.toml` otherwise
///
/// Returns [`Config::default()`] if the file does not exist.
/// Logs a warning and returns default if the file exists but cannot be parsed.
pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Config::default(),
        Err(e) => {
            eprintln!(
                "[sift] warning: could not read config at {}: {e}",
                path.display()
            );
            return Config::default();
        }
    };

    match toml::from_str(&content) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "[sift] warning: could not parse config at {}: {e}",
                path.display()
            );
            Config::default()
        }
    }
}

/// Resolve the config file path.
///
/// Uses `$XDG_CONFIG_HOME` if set, otherwise falls back to `$HOME/.config`.
fn config_path() -> Option<std::path::PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(
                std::path::PathBuf::from(xdg)
                    .join("sift")
                    .join("config.toml"),
            );
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join(".config")
            .join("sift")
            .join("config.toml"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::Verbosity;

    #[test]
    fn parse_verbosity_maps_known_values() {
        assert!(matches!(parse_verbosity("compact"), Verbosity::Compact));
        assert!(matches!(parse_verbosity("verbose"), Verbosity::Verbose));
        assert!(matches!(
            parse_verbosity("very_verbose"),
            Verbosity::VeryVerbose
        ));
        assert!(matches!(
            parse_verbosity("very-verbose"),
            Verbosity::VeryVerbose
        ));
        assert!(matches!(parse_verbosity("maximum"), Verbosity::Maximum));
        assert!(matches!(parse_verbosity("raw"), Verbosity::Raw));
    }

    #[test]
    fn parse_verbosity_unknown_falls_back_to_compact() {
        assert!(matches!(parse_verbosity(""), Verbosity::Compact));
        assert!(matches!(parse_verbosity("bogus"), Verbosity::Compact));
    }

    #[test]
    fn config_from_toml_uses_defaults_for_missing_fields() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.defaults.verbosity, "compact");
        assert_eq!(cfg.defaults.max_lines, 100);
        assert!(cfg.tracking.enabled);
    }

    #[test]
    fn config_from_toml_parses_verbosity_override() {
        let toml_str = "[defaults]\nverbosity = \"verbose\"\nmax_lines = 50\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.defaults.verbosity, "verbose");
        assert_eq!(cfg.defaults.max_lines, 50);
    }

    #[test]
    fn config_from_toml_parses_tracking_disabled() {
        let toml_str = "[tracking]\nenabled = false\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(!cfg.tracking.enabled);
    }

    #[test]
    fn tee_enabled_by_default() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.tee.enabled);
    }

    #[test]
    fn config_from_toml_parses_tee_disabled() {
        let toml_str = "[tee]\nenabled = false\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(!cfg.tee.enabled);
    }

    // --- Per-command config tests ---

    #[test]
    fn per_command_config_parses_verbosity() {
        let toml_str = "[commands.git]\nverbosity = \"verbose\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        let git = cfg.commands.get("git").expect("git entry missing");
        assert_eq!(git.verbosity, "verbose");
    }

    #[test]
    fn per_command_config_parses_max_lines() {
        let toml_str = "[commands.xcodebuild]\nverbosity = \"compact\"\nmax_lines = 30\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        let xb = cfg.commands.get("xcodebuild").expect("xcodebuild entry missing");
        assert_eq!(xb.verbosity, "compact");
        assert_eq!(xb.max_lines, 30);
    }

    #[test]
    fn resolve_verbosity_cli_wins_over_command_override() {
        let toml_str = "[commands.git]\nverbosity = \"verbose\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        // CLI --raw should override the per-command "verbose"
        let v = cfg.resolve_verbosity("git", Some(Verbosity::Raw));
        assert!(matches!(v, Verbosity::Raw));
    }

    #[test]
    fn resolve_verbosity_command_override_takes_precedence_over_global_default() {
        let toml_str =
            "[defaults]\nverbosity = \"compact\"\n\n[commands.git]\nverbosity = \"verbose\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        let v = cfg.resolve_verbosity("git", None);
        assert!(matches!(v, Verbosity::Verbose));
    }

    #[test]
    fn resolve_verbosity_falls_back_to_global_default_when_no_command_override() {
        let toml_str = "[defaults]\nverbosity = \"very_verbose\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        let v = cfg.resolve_verbosity("git", None);
        assert!(matches!(v, Verbosity::VeryVerbose));
    }

    #[test]
    fn resolve_verbosity_falls_back_to_compact_when_nothing_set() {
        let cfg: Config = toml::from_str("").unwrap();
        let v = cfg.resolve_verbosity("git", None);
        assert!(matches!(v, Verbosity::Compact));
    }

    #[test]
    fn resolve_verbosity_unknown_command_uses_global_default() {
        let toml_str = "[defaults]\nverbosity = \"verbose\"\n\n[commands.git]\nverbosity = \"raw\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        // "grep" has no override — should fall back to global "verbose"
        let v = cfg.resolve_verbosity("grep", None);
        assert!(matches!(v, Verbosity::Verbose));
    }

    #[test]
    fn per_command_config_empty_verbosity_falls_back_to_global() {
        // An entry with empty verbosity should not override the global default
        let toml_str =
            "[defaults]\nverbosity = \"verbose\"\n\n[commands.git]\nmax_lines = 50\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        let v = cfg.resolve_verbosity("git", None);
        assert!(matches!(v, Verbosity::Verbose));
    }
}
