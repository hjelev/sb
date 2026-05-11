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
