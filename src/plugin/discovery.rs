//! Plugin discovery: scan the plugins directory for Lua scripts.

use std::path::{Path, PathBuf};

use super::PluginSource;
use crate::util::config::SbPersistConfig;

/// The default plugins directory: `~/.config/sb/plugins`.
pub(crate) fn plugins_dir() -> PathBuf {
    crate::util::config::config_dir().join("plugins")
}

/// Example plugins shipped inside the binary, so installs that only deliver
/// the executable (cargo, Homebrew, the shell installer) still get them.
/// Seeded into [`plugins_dir`] once per plugin by [`seed_bundled_plugins`].
const BUNDLED_PLUGINS: &[(&str, &str)] = &[
    ("cdlog", include_str!("../../examples/plugins/cdlog.lua")),
    ("duh", include_str!("../../examples/plugins/duh.lua")),
    (
        "git-commit",
        include_str!("../../examples/plugins/git-commit.lua"),
    ),
    (
        "linecount",
        include_str!("../../examples/plugins/linecount.lua"),
    ),
    (
        "touch-notify",
        include_str!("../../examples/plugins/touch-notify.lua"),
    ),
];

/// Bundled plugins that change behavior just by being loaded (event hooks
/// that write files, a previewer that takes over `.txt`/`.log`). They are
/// seeded disabled; the user opts in from the Plugins panel (`P`). The rest
/// only expose `entry()` commands and do nothing until invoked.
const SEED_DISABLED: &[&str] = &["cdlog", "linecount"];

/// Copies each bundled plugin into `dir` as `<name>.lua` unless it was seeded
/// before (recorded in `cfg.seeded_plugins`, so a deleted plugin stays
/// deleted) or a plugin of that name already exists in either form. Returns
/// true when `cfg` changed and should be saved.
pub(crate) fn seed_bundled_plugins(dir: &Path, cfg: &mut SbPersistConfig) -> bool {
    let pending: Vec<&(&str, &str)> = BUNDLED_PLUGINS
        .iter()
        .filter(|(name, _)| !cfg.seeded_plugins.iter().any(|s| s == name))
        .collect();
    if pending.is_empty() || std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let mut changed = false;
    for (name, content) in pending {
        let flat = dir.join(format!("{name}.lua"));
        let exists = flat.exists() || dir.join(name).join("main.lua").exists();
        if !exists {
            if std::fs::write(&flat, content).is_err() {
                // Leave it unrecorded so the next start retries.
                continue;
            }
            if SEED_DISABLED.contains(name) && !cfg.disabled_plugins.iter().any(|d| d == name) {
                cfg.disabled_plugins.push(name.to_string());
            }
        }
        cfg.seeded_plugins.push(name.to_string());
        changed = true;
    }
    changed
}

/// Per-plugin state directory (`~/.config/sb/plugin-data/<name>`), exposed to
/// scripts as `sb.data_dir()`. Created lazily by the API call, not here.
pub(crate) fn plugin_data_dir(name: &str) -> PathBuf {
    crate::util::config::config_dir()
        .join("plugin-data")
        .join(name)
}

/// Scan `dir` for plugins. Accepts `<name>/main.lua` (directory form) and
/// `<name>.lua` (flat form); the directory form shadows a flat file with the
/// same name. Names that would break the `plugin_key_<name> = ...` config
/// line format (whitespace, `=`, or empty) are skipped. Results are sorted
/// by name for stable ordering.
pub(crate) fn discover(dir: &Path) -> Vec<PluginSource> {
    let mut found: Vec<PluginSource> = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return found,
    };
    let mut flat: Vec<PluginSource> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.is_dir() {
            let script = path.join("main.lua");
            if script.is_file() && valid_name(file_name) {
                found.push(PluginSource {
                    name: file_name.to_string(),
                    script,
                });
            }
        } else if let Some(stem) = file_name.strip_suffix(".lua")
            && valid_name(stem)
        {
            flat.push(PluginSource {
                name: stem.to_string(),
                script: path,
            });
        }
    }
    for src in flat {
        if !found.iter().any(|p| p.name == src.name) {
            found.push(src);
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(|c: char| c.is_whitespace() || c == '=')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_bundled_plugins_once_and_respects_deletions() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("plugins");
        // A user-authored plugin with a bundled name must not be overwritten.
        std::fs::create_dir_all(dir.join("duh")).unwrap();
        std::fs::write(dir.join("duh/main.lua"), "-- mine").unwrap();

        let mut cfg = SbPersistConfig::default();
        assert!(seed_bundled_plugins(&dir, &mut cfg));
        for (name, _) in BUNDLED_PLUGINS {
            assert!(cfg.seeded_plugins.iter().any(|s| s == name), "{name}");
        }
        assert!(!dir.join("duh.lua").exists());
        assert!(dir.join("git-commit.lua").is_file());
        assert_eq!(cfg.disabled_plugins, vec!["cdlog", "linecount"]);

        // Second run is a no-op, and a deleted plugin is not restored.
        std::fs::remove_file(dir.join("touch-notify.lua")).unwrap();
        assert!(!seed_bundled_plugins(&dir, &mut cfg));
        assert!(!dir.join("touch-notify.lua").exists());

        // All seeded plugins load without errors.
        let names: Vec<String> = discover(&dir).into_iter().map(|p| p.name).collect();
        assert!(names.contains(&"git-commit".to_string()));
    }

    #[test]
    fn discovers_flat_and_dir_forms_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("zeta.lua"), "return {}").unwrap();
        std::fs::create_dir(dir.join("alpha")).unwrap();
        std::fs::write(dir.join("alpha/main.lua"), "return {}").unwrap();

        let found = discover(dir);
        let names: Vec<&str> = found.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zeta"]);
        assert!(found[0].script.ends_with("alpha/main.lua"));
        assert!(found[1].script.ends_with("zeta.lua"));
    }

    #[test]
    fn dir_form_shadows_flat_form() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("dup.lua"), "return {}").unwrap();
        std::fs::create_dir(dir.join("dup")).unwrap();
        std::fs::write(dir.join("dup/main.lua"), "return {}").unwrap();

        let found = discover(dir);
        assert_eq!(found.len(), 1);
        assert!(found[0].script.ends_with("dup/main.lua"));
    }

    #[test]
    fn ignores_non_lua_invalid_names_and_dirs_without_main() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("readme.txt"), "hi").unwrap();
        std::fs::write(dir.join("bad name.lua"), "return {}").unwrap();
        std::fs::create_dir(dir.join("empty")).unwrap();

        assert!(discover(dir).is_empty());
    }

    #[test]
    fn missing_dir_yields_empty() {
        assert!(discover(Path::new("/nonexistent/sb-plugins")).is_empty());
    }
}
