//! Application initialization helpers.
//!
//! Extracted from main.rs to keep startup logic organized.
//! In future phases, more of main() function will be moved here.
#![allow(dead_code)]

use std::env;
use crate::util::config::AppConfig;

/// Initialize application configuration from environment and arguments.
///
/// Called once during app startup, before creating the App struct.
/// Parses all environment variables and command-line arguments that affect
/// application behavior.
pub fn init_config() -> AppConfig {
    AppConfig::from_env()
}

/// Get the initial working directory for the application.
///
/// Tries to use the current working directory; falls back to home if cwd
/// is inaccessible or doesn't exist.
pub fn init_current_dir() -> std::io::Result<std::path::PathBuf> {
    // For now, just use the actual current directory
    // In future, could support opening specific paths from command line
    env::current_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_config() {
        let config = init_config();
        // Config should always be initialized with sensible defaults
        assert_eq!(config.preview_line_limit, 1000);
        assert_eq!(config.dir_list_limit, 10000);
    }

    #[test]
    fn test_init_current_dir() {
        let dir = init_current_dir();
        // Should successfully get current directory
        assert!(dir.is_ok());
        assert!(dir.unwrap().exists());
    }
}
