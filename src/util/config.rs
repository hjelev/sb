//! Centralized application configuration.
//!
//! Parses environment variables and provides a singleton configuration struct
//! used throughout the application. This replaces scattered env reads throughout
//! the codebase.
#![allow(dead_code)]

use std::env;

use crate::FilenameColorMode;

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
            editor: crate::util::command::editor_command(),
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

/// Returns the sb config directory: `$XDG_CONFIG_HOME/sb` or `~/.config/sb`
/// if the env var is unset.
pub fn config_dir() -> std::path::PathBuf {
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
    base.join("sb")
}

/// Returns the sb runtime directory for transient state such as remote mount
/// points: `$XDG_RUNTIME_DIR/sb`, falling back to `$XDG_CACHE_HOME/sb` or
/// `~/.cache/sb`. Unlike a predictable `/tmp/...` path, these bases are
/// per-user and not world-writable, which avoids the symlink/TOCTOU exposure of
/// a shared, guessable mount path.
pub fn runtime_dir() -> std::path::PathBuf {
    let base = env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            env::var("XDG_CACHE_HOME")
                .ok()
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from)
        })
        .or_else(|| {
            env::var("HOME")
                .ok()
                .map(|h| std::path::PathBuf::from(h).join(".cache"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from(".cache"));
    base.join("sb")
}

/// Returns the path to the persistent config file: `$XDG_CONFIG_HOME/sb/config`
/// or `~/.config/sb/config` if the env var is unset.
fn persist_config_path() -> std::path::PathBuf {
    config_dir().join("config")
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
    /// The active UI theme to restore on next launch.
    pub current_theme: String,
    /// Nerd Font glyph mode. `None` means unset (fall back to the
    /// `NERD_FONT_ACTIVE` env var); `Some` overrides the env var.
    pub nerd_font: Option<bool>,
    /// When `Some(true)`, the header clock is replaced by the disk-usage pill.
    /// `None` means unset (clock shown by default).
    pub disable_clock: Option<bool>,
    /// How file (not folder) names are colored in the list.
    pub(crate) filename_color_mode: FilenameColorMode,
    /// Whether folder size calculation (the `s` toggle) is enabled on launch.
    pub folder_size_enabled: bool,
    /// Integration keys that the user has explicitly disabled.
    pub disabled_integrations: Vec<String>,
    /// Persistent bookmarks (index 0–9 → path string). Env vars take precedence at runtime.
    pub bookmarks: std::collections::HashMap<u8, String>,
    /// Unknown settings (future-proofing: preserve any unrecognized key-value pairs).
    unknown: std::collections::HashMap<String, String>,
}

impl Default for SbPersistConfig {
    fn default() -> Self {
        Self {
            view_mode: "Normal".to_string(),
            current_theme: "original".to_string(),
            nerd_font: None,
            disable_clock: None,
            filename_color_mode: FilenameColorMode::Full,
            folder_size_enabled: false,
            disabled_integrations: Vec::new(),
            bookmarks: std::collections::HashMap::new(),
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
                    "current_theme" => cfg.current_theme = val.to_string(),
                    "nerd_font" => {
                        cfg.nerd_font = match val.to_ascii_lowercase().as_str() {
                            "1" | "true" => Some(true),
                            "0" | "false" => Some(false),
                            _ => None,
                        };
                    }
                    "disable_clock" => {
                        cfg.disable_clock = match val.to_ascii_lowercase().as_str() {
                            "1" | "true" => Some(true),
                            "0" | "false" => Some(false),
                            _ => None,
                        };
                    }
                    "filename_colors" => {
                        cfg.filename_color_mode = FilenameColorMode::from_key(val);
                    }
                    "folder_size_enabled" => {
                        cfg.folder_size_enabled = matches!(
                            val.to_ascii_lowercase().as_str(),
                            "1" | "true"
                        );
                    }
                    "disabled_integrations" => {
                        cfg.disabled_integrations = val
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect();
                    }
                    k if k.starts_with("bookmark_") => {
                        if let Ok(n) = k["bookmark_".len()..].parse::<u8>()
                            && n <= 9 && !val.is_empty() {
                                cfg.bookmarks.insert(n, val.to_string());
                            }
                    }
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
        lines.push(format!("current_theme = {}", self.current_theme));
        if let Some(nf) = self.nerd_font {
            lines.push(format!("nerd_font = {}", nf));
        }
        if let Some(dc) = self.disable_clock {
            lines.push(format!("disable_clock = {}", dc));
        }
        lines.push(format!("filename_colors = {}", self.filename_color_mode.as_key()));
        lines.push(format!("folder_size_enabled = {}", self.folder_size_enabled));
        if !self.disabled_integrations.is_empty() {
            lines.push(format!(
                "disabled_integrations = {}",
                self.disabled_integrations.join(",")
            ));
        }
        let mut sorted_bookmarks: Vec<(&u8, &String)> = self.bookmarks.iter().collect();
        sorted_bookmarks.sort_by_key(|(n, _)| *n);
        for (n, path) in sorted_bookmarks {
            lines.push(format!("bookmark_{} = {}", n, path));
        }
        for (key, val) in &self.unknown {
            lines.push(format!("{} = {}", key, val));
        }
        let content = lines.join("\n") + "\n";
        std::fs::write(&path, content)
    }

    /// Load the persisted config, apply `f` to it, and save the result back to
    /// disk. Centralizes the common load → mutate → save dance so callers that
    /// only flip a single field don't have to repeat it. Save errors are
    /// ignored, matching the existing `let _ = cfg.save();` call sites.
    pub fn update(f: impl FnOnce(&mut Self)) {
        let mut cfg = Self::load();
        f(&mut cfg);
        let _ = cfg.save();
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

    #[test]
    fn test_filename_color_mode_key_round_trip() {
        for mode in [
            FilenameColorMode::Full,
            FilenameColorMode::Less,
            FilenameColorMode::White,
        ] {
            assert_eq!(FilenameColorMode::from_key(mode.as_key()), mode);
        }
        // Unknown / legacy values fall back to Full.
        assert_eq!(FilenameColorMode::from_key("bogus"), FilenameColorMode::Full);
        assert_eq!(FilenameColorMode::from_key(""), FilenameColorMode::Full);
        // Default config uses Full.
        assert_eq!(SbPersistConfig::default().filename_color_mode, FilenameColorMode::Full);
    }

    #[test]
    fn test_bookmark_serialization_round_trip() {
        let mut cfg = SbPersistConfig::default();
        cfg.bookmarks.insert(0, "/tmp/test0".to_string());
        cfg.bookmarks.insert(3, "/home/user/projects".to_string());
        cfg.bookmarks.insert(9, "/var/log".to_string());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        // Simulate what save() would write
        let mut lines = vec!["# sb config".to_string()];
        lines.push(format!("view_mode = {}", cfg.view_mode));
        lines.push(format!("current_theme = {}", cfg.current_theme));
        let mut sorted: Vec<(&u8, &String)> = cfg.bookmarks.iter().collect();
        sorted.sort_by_key(|(n, _)| *n);
        for (n, bpath) in &sorted {
            lines.push(format!("bookmark_{} = {}", n, bpath));
        }
        let content = lines.join("\n") + "\n";
        std::fs::write(&path, &content).unwrap();

        // Parse it back
        let loaded = content.lines().fold(SbPersistConfig::default(), |mut c, line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { return c; }
            if let Some((k, v)) = line.split_once('=') {
                let k = k.trim(); let v = v.trim();
                if k.starts_with("bookmark_") {
                    if let Ok(n) = k["bookmark_".len()..].parse::<u8>() {
                        if n <= 9 && !v.is_empty() { c.bookmarks.insert(n, v.to_string()); }
                    }
                }
            }
            c
        });

        assert_eq!(loaded.bookmarks.get(&0), Some(&"/tmp/test0".to_string()));
        assert_eq!(loaded.bookmarks.get(&3), Some(&"/home/user/projects".to_string()));
        assert_eq!(loaded.bookmarks.get(&9), Some(&"/var/log".to_string()));
    }
}
