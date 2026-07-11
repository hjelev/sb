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

/// Returns the path used to hand the last-visited directory to the optional
/// shell `cd`-on-exit integration (see README). Lives under [`runtime_dir`]
/// (per-user, not world-writable) rather than a fixed `/tmp/...` name, to
/// avoid the symlink/TOCTOU and cross-user-visibility exposure of a shared,
/// guessable path.
pub fn last_path_file() -> std::path::PathBuf {
    runtime_dir().join("last_path")
}

/// Restrict `path` to owner-only read/write (`0600`) on Unix. Used for files
/// that may contain secrets (API keys) or otherwise shouldn't be readable by
/// other local users. No-op on non-Unix platforms.
fn restrict_to_owner(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
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
    /// AI commit-message provider key: `"groq"` or `"github"`.
    pub ai_provider: String,
    /// AI model id; empty falls back to the provider's default at call time.
    pub ai_model: String,
    /// AI API keys/tokens, one per provider (keyed by provider key, e.g.
    /// `"groq"` / `"github"`). A missing/empty entry falls back to the
    /// provider's env var at call time.
    pub ai_api_keys: std::collections::HashMap<String, String>,
    /// When true, the commit prompt auto-generates an AI message on open
    /// (no Ctrl+G needed).
    pub ai_auto_commit: bool,
    /// Custom keyboard shortcuts (action id → combo string, e.g.
    /// `"rename" → "u"`). Only non-default bindings are stored; ids and
    /// combo syntax live in [`crate::util::keymap`].
    pub shortcuts: std::collections::HashMap<String, String>,
    /// Plugin names the user has explicitly disabled.
    pub disabled_plugins: Vec<String>,
    /// Key bindings for plugin commands (plugin name → combo string, stored
    /// as `plugin_key_<name> = <combo>`). Combo syntax matches shortcuts.
    pub plugin_bindings: std::collections::HashMap<String, String>,
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
            ai_provider: "groq".to_string(),
            ai_model: String::new(),
            ai_api_keys: std::collections::HashMap::new(),
            ai_auto_commit: false,
            shortcuts: std::collections::HashMap::new(),
            disabled_plugins: Vec::new(),
            plugin_bindings: std::collections::HashMap::new(),
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
        // Legacy single `ai_api_key` value, resolved to a provider after the
        // parse loop (so it is independent of line order relative to
        // `ai_provider`).
        let mut legacy_ai_api_key: Option<String> = None;
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
                    "ai_provider" => cfg.ai_provider = val.to_string(),
                    "ai_model" => cfg.ai_model = val.to_string(),
                    "ai_api_key" => legacy_ai_api_key = Some(val.to_string()),
                    k if k.starts_with("ai_api_key_") => {
                        let provider = &k["ai_api_key_".len()..];
                        if !provider.is_empty() && !val.is_empty() {
                            cfg.ai_api_keys.insert(provider.to_string(), val.to_string());
                        }
                    }
                    "ai_auto_commit" => {
                        cfg.ai_auto_commit = matches!(
                            val.to_ascii_lowercase().as_str(),
                            "1" | "true"
                        );
                    }
                    k if k.starts_with("shortcut_") => {
                        let id = &k["shortcut_".len()..];
                        if !id.is_empty() && !val.is_empty() {
                            cfg.shortcuts.insert(id.to_string(), val.to_string());
                        }
                    }
                    "disabled_plugins" => {
                        cfg.disabled_plugins = val
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect();
                    }
                    k if k.starts_with("plugin_key_") => {
                        let name = &k["plugin_key_".len()..];
                        if !name.is_empty() && !val.is_empty() {
                            cfg.plugin_bindings.insert(name.to_string(), val.to_string());
                        }
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
        // Migrate a legacy single key into the active provider's slot, unless
        // that provider already has a per-provider key.
        if let Some(key) = legacy_ai_api_key
            && !key.is_empty() {
                cfg.ai_api_keys
                    .entry(cfg.ai_provider.clone())
                    .or_insert(key);
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
        lines.push(format!("ai_provider = {}", self.ai_provider));
        lines.push(format!("ai_auto_commit = {}", self.ai_auto_commit));
        if !self.ai_model.is_empty() {
            lines.push(format!("ai_model = {}", self.ai_model));
        }
        let mut sorted_keys: Vec<(&String, &String)> = self.ai_api_keys.iter().collect();
        sorted_keys.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (provider, key) in sorted_keys {
            if !key.is_empty() {
                lines.push(format!("ai_api_key_{} = {}", provider, key));
            }
        }
        let mut sorted_shortcuts: Vec<(&String, &String)> = self.shortcuts.iter().collect();
        sorted_shortcuts.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (id, combo) in sorted_shortcuts {
            if !combo.is_empty() {
                lines.push(format!("shortcut_{} = {}", id, combo));
            }
        }
        if !self.disabled_plugins.is_empty() {
            lines.push(format!(
                "disabled_plugins = {}",
                self.disabled_plugins.join(",")
            ));
        }
        let mut sorted_plugin_bindings: Vec<(&String, &String)> =
            self.plugin_bindings.iter().collect();
        sorted_plugin_bindings.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (name, combo) in sorted_plugin_bindings {
            if !combo.is_empty() {
                lines.push(format!("plugin_key_{} = {}", name, combo));
            }
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
        std::fs::write(&path, content)?;
        // Config may contain plaintext AI provider API keys; keep it unreadable
        // by other local users regardless of the process umask.
        restrict_to_owner(&path)
    }

    /// Load the persisted config, apply `f` to it, and save the result back to
    /// disk. Centralizes the common load → mutate → save dance so callers that
    /// only flip a single field don't have to repeat it. Returns the save
    /// result; callers persisting cosmetic state may ignore it, but anything
    /// the user typed (e.g. API keys) should surface a failure.
    pub fn update(f: impl FnOnce(&mut Self)) -> std::io::Result<()> {
        let mut cfg = Self::load();
        f(&mut cfg);
        cfg.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_restrict_to_owner_sets_0600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        std::fs::write(&path, "ai_api_key_groq = secret\n").unwrap();
        restrict_to_owner(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

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
                if k.starts_with("bookmark_")
                    && let Ok(n) = k["bookmark_".len()..].parse::<u8>()
                        && n <= 9 && !v.is_empty() { c.bookmarks.insert(n, v.to_string()); }
            }
            c
        });

        assert_eq!(loaded.bookmarks.get(&0), Some(&"/tmp/test0".to_string()));
        assert_eq!(loaded.bookmarks.get(&3), Some(&"/home/user/projects".to_string()));
        assert_eq!(loaded.bookmarks.get(&9), Some(&"/var/log".to_string()));
    }

    #[test]
    fn test_shortcut_serialization_round_trip() {
        let mut cfg = SbPersistConfig::default();
        cfg.shortcuts.insert("rename".to_string(), "u".to_string());
        cfg.shortcuts.insert("sort_menu".to_string(), "ctrl+t".to_string());

        // Simulate what save() writes for shortcuts (sorted by id).
        let mut sorted: Vec<(&String, &String)> = cfg.shortcuts.iter().collect();
        sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
        let content = sorted
            .iter()
            .map(|(id, combo)| format!("shortcut_{} = {}", id, combo))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(content, "shortcut_rename = u\nshortcut_sort_menu = ctrl+t");

        // Parse it back with load()'s prefix logic.
        let loaded = content.lines().fold(SbPersistConfig::default(), |mut c, line| {
            if let Some((k, v)) = line.split_once('=') {
                let (k, v) = (k.trim(), v.trim());
                if let Some(id) = k.strip_prefix("shortcut_")
                    && !id.is_empty() && !v.is_empty() {
                        c.shortcuts.insert(id.to_string(), v.to_string());
                    }
            }
            c
        });
        assert_eq!(loaded.shortcuts.get("rename"), Some(&"u".to_string()));
        assert_eq!(loaded.shortcuts.get("sort_menu"), Some(&"ctrl+t".to_string()));
    }

    #[test]
    fn test_plugin_settings_round_trip() {
        let mut cfg = SbPersistConfig::default();
        cfg.disabled_plugins = vec!["cdlog".to_string(), "linecount".to_string()];
        cfg.plugin_bindings
            .insert("touch-notify".to_string(), "ctrl+t".to_string());

        // Simulate what save() writes for plugin settings.
        let mut lines = Vec::new();
        lines.push(format!(
            "disabled_plugins = {}",
            cfg.disabled_plugins.join(",")
        ));
        let mut sorted: Vec<(&String, &String)> = cfg.plugin_bindings.iter().collect();
        sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (name, combo) in sorted {
            lines.push(format!("plugin_key_{} = {}", name, combo));
        }
        let content = lines.join("\n");
        assert_eq!(
            content,
            "disabled_plugins = cdlog,linecount\nplugin_key_touch-notify = ctrl+t"
        );

        // Parse it back with load()'s prefix logic.
        let loaded = content.lines().fold(SbPersistConfig::default(), |mut c, line| {
            if let Some((k, v)) = line.split_once('=') {
                let (k, v) = (k.trim(), v.trim());
                if k == "disabled_plugins" {
                    c.disabled_plugins = v
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect();
                } else if let Some(name) = k.strip_prefix("plugin_key_")
                    && !name.is_empty() && !v.is_empty() {
                        c.plugin_bindings.insert(name.to_string(), v.to_string());
                    }
            }
            c
        });
        assert_eq!(loaded.disabled_plugins, vec!["cdlog", "linecount"]);
        assert_eq!(
            loaded.plugin_bindings.get("touch-notify"),
            Some(&"ctrl+t".to_string())
        );
    }

    /// Reproduce `load()`'s parse of one config line into `cfg`, including the
    /// per-provider `ai_api_key_*` and legacy `ai_api_key` handling. Legacy
    /// migration is applied by the caller after all lines are folded.
    fn parse_line_into(mut c: SbPersistConfig, line: &str, legacy: &mut Option<String>)
        -> SbPersistConfig {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return c;
        }
        if let Some((k, v)) = line.split_once('=') {
            let (k, v) = (k.trim(), v.trim());
            match k {
                "ai_provider" => c.ai_provider = v.to_string(),
                "ai_model" => c.ai_model = v.to_string(),
                "ai_api_key" => *legacy = Some(v.to_string()),
                k if k.starts_with("ai_api_key_") => {
                    let provider = &k["ai_api_key_".len()..];
                    if !provider.is_empty() && !v.is_empty() {
                        c.ai_api_keys.insert(provider.to_string(), v.to_string());
                    }
                }
                "ai_auto_commit" => {
                    c.ai_auto_commit =
                        matches!(v.to_ascii_lowercase().as_str(), "1" | "true");
                }
                _ => {}
            }
        }
        c
    }

    #[test]
    fn test_ai_settings_round_trip() {
        // Default provider is groq; model/keys empty so they fall back at runtime.
        let def = SbPersistConfig::default();
        assert_eq!(def.ai_provider, "groq");
        assert!(def.ai_model.is_empty() && def.ai_api_keys.is_empty());
        assert!(!def.ai_auto_commit);

        // Separate per-provider keys are written as `ai_api_key_<provider>` and
        // parsed back into the map.
        let mut cfg = SbPersistConfig {
            ai_provider: "github".to_string(),
            ..Default::default()
        };
        cfg.ai_model = "openai/gpt-4o-mini".to_string();
        cfg.ai_api_keys.insert("groq".to_string(), "groq-secret".to_string());
        cfg.ai_api_keys.insert("github".to_string(), "github-secret".to_string());
        cfg.ai_auto_commit = true;

        let content = {
            let mut lines = vec!["# sb config".to_string()];
            lines.push(format!("ai_provider = {}", cfg.ai_provider));
            lines.push(format!("ai_auto_commit = {}", cfg.ai_auto_commit));
            if !cfg.ai_model.is_empty() {
                lines.push(format!("ai_model = {}", cfg.ai_model));
            }
            let mut sorted: Vec<(&String, &String)> = cfg.ai_api_keys.iter().collect();
            sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (provider, key) in sorted {
                lines.push(format!("ai_api_key_{} = {}", provider, key));
            }
            lines.join("\n") + "\n"
        };

        let mut legacy = None;
        let loaded = content
            .lines()
            .fold(SbPersistConfig::default(), |c, line| {
                parse_line_into(c, line, &mut legacy)
            });

        assert_eq!(loaded.ai_provider, "github");
        assert_eq!(loaded.ai_model, "openai/gpt-4o-mini");
        assert_eq!(loaded.ai_api_keys.get("groq"), Some(&"groq-secret".to_string()));
        assert_eq!(
            loaded.ai_api_keys.get("github"),
            Some(&"github-secret".to_string())
        );
        assert!(loaded.ai_auto_commit);
    }

    #[test]
    fn test_legacy_ai_api_key_migrates_to_active_provider() {
        // An old config with a single `ai_api_key` line should migrate that key
        // into the active provider's slot on load.
        let content = "ai_provider = github\nai_api_key = old-token\n";
        let mut legacy = None;
        let mut loaded = content
            .lines()
            .fold(SbPersistConfig::default(), |c, line| {
                parse_line_into(c, line, &mut legacy)
            });
        // Apply the same post-loop migration as load().
        if let Some(key) = legacy
            && !key.is_empty() {
                loaded
                    .ai_api_keys
                    .entry(loaded.ai_provider.clone())
                    .or_insert(key);
            }
        assert_eq!(
            loaded.ai_api_keys.get("github"),
            Some(&"old-token".to_string())
        );
    }
}
