//! Centralized application configuration.
//!
//! Parses environment variables and provides a singleton configuration struct
//! used throughout the application. This replaces scattered env reads throughout
//! the codebase.
#![allow(dead_code)]

use std::env;

/// Application-wide configuration parsed from environment and defaults.
///
/// This struct should be created once at startup and stored in the `App` struct.
/// Access via `app.config` or pass `&config` to methods that need it.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Enable Nerd Font icons (via NERD_FONT_ACTIVE=1 env var)
    pub nerd_font_active: bool,

    /// Disable colored output (via NO_COLOR env var)
    pub no_color: bool,

    /// Show file-type icons in list view (via TERMINAL_ICONS; defaults to true)
    pub show_icons: bool,

    /// Editor command for opening files (via EDITOR; fallback: "nano")
    pub editor: String,

    /// Maximum preview lines to render (default: 1000)
    pub preview_line_limit: usize,

    /// Maximum files to list in a directory (default: 10000)
    pub dir_list_limit: usize,

    /// File size threshold for binary detection (default: 10*1024*1024 bytes = 10 MB)
    pub binary_threshold: u64,

    /// Search candidate limit (default: 5000)
    pub search_candidate_limit: usize,

    /// Search content lines limit (default: 500)
    pub search_content_line_limit: usize,

    /// Database preview row limit (default: 1000)
    pub db_preview_row_limit: usize,
}

impl AppConfig {
    /// Parse configuration from environment variables.
    ///
    /// Called once during app initialization (in `app_init::init_app()`).
    /// Returns a config struct with values read from env or sensible defaults.
    pub fn from_env() -> Self {
        Self {
            nerd_font_active: env::var("NERD_FONT_ACTIVE")
                .map(|v| v == "1")
                .unwrap_or(false),
            no_color: env_flag_true(&["NO_COLOR"]),
            show_icons: env::var("TERMINAL_ICONS")
                .map(|v| v != "0")
                .unwrap_or(true),
            editor: env::var("EDITOR").unwrap_or_else(|_| "nano".to_string()),
            preview_line_limit: 1000,
            dir_list_limit: 10000,
            binary_threshold: 10 * 1024 * 1024,
            search_candidate_limit: 5000,
            search_content_line_limit: 500,
            db_preview_row_limit: 1000,
        }
    }
}

/// Check if any of the given environment variable names is set to true.
///
/// Handles NO_COLOR specially: if NO_COLOR is set to a falsey value,
/// removes it from the environment to prevent downstream leakage.
fn env_flag_true(names: &[&str]) -> bool {
    for name in names {
        if let Ok(raw) = env::var(name) {
            let v = raw.trim();
            let is_true = v == "1" || v.eq_ignore_ascii_case("true");
            if !is_true && *name == "NO_COLOR" {
                // SAFETY: This runs during startup before any worker threads are spawned,
                // so mutating the process environment here avoids races while ensuring
                // falsey NO_COLOR values do not leak through.
                unsafe {
                    env::remove_var(name);
                }
            }
            return is_true;
        }
    }
    false
}

/// Returns the path to the persistent config file: `$XDG_CONFIG_HOME/sb/config`
/// or `~/.config/sb/config` if the env var is unset.
fn persist_config_path() -> std::path::PathBuf {
    let base = env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            env::var("HOME")
                .ok()
                .map(|h| std::path::PathBuf::from(h).join(".config"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from(".config"));
    base.join("sb").join("config")
}

/// Persistent (file-based) configuration stored in `~/.config/sb/config`.
///
/// This is separate from [`AppConfig`] (which is env-only / runtime flags).
/// Settings here survive across application restarts. Unknown settings are preserved
/// for forward compatibility (allowing new settings to be added without loss).
#[derive(Debug, Clone)]
pub struct SbPersistConfig {
    /// The view mode to restore on next launch: `"Normal"`, `"Preview"`, or `"DualPanel"`.
    pub view_mode: String,
    /// Unknown settings (future-proofing: preserve any unrecognized key-value pairs).
    unknown: std::collections::HashMap<String, String>,
}

impl Default for SbPersistConfig {
    fn default() -> Self {
        Self {
            view_mode: "Normal".to_string(),
            unknown: std::collections::HashMap::new(),
        }
    }
}

impl SbPersistConfig {
    /// Load persistent config from disk. Falls back to defaults on any error.
    /// Preserves any unknown settings for future compatibility.
    pub fn load() -> Self {
        let path = persist_config_path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let mut cfg = Self::default();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "view_mode" => cfg.view_mode = val.to_string(),
                    _ => {
                        cfg.unknown.insert(key.to_string(), val.to_string());
                    }
                }
            }
        }
        cfg
    }

    /// Save persistent config to disk. Creates parent directories as needed.
    /// Only updates view_mode; all other settings are preserved.
    pub fn save(&self) -> std::io::Result<()> {
        let path = persist_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut lines = vec!["# sb config".to_string()];
        lines.push(format!("view_mode = {}", self.view_mode));
        for (key, val) in &self.unknown {
            lines.push(format!("{} = {}", key, val));
        }
        let content = lines.join("\n") + "\n";
        std::fs::write(&path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env_defaults() {
        let config = AppConfig::from_env();
        // These values should always be present even if env vars are unset
        assert_eq!(config.preview_line_limit, 1000);
        assert_eq!(config.dir_list_limit, 10000);
        assert_eq!(config.binary_threshold, 10 * 1024 * 1024);
    }
}
