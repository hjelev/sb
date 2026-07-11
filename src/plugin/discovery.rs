//! Plugin discovery: scan the plugins directory for Lua scripts.

use std::path::{Path, PathBuf};

use super::PluginSource;

/// The default plugins directory: `~/.config/sb/plugins`.
pub(crate) fn plugins_dir() -> PathBuf {
    crate::util::config::config_dir().join("plugins")
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
