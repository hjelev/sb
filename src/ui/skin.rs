//! External theme ("skin") support.
//!
//! Loads Midnight-Commander–style `.ini` skin files from `~/.config/sb/skins/`
//! and converts them into [`ThemeSpec`]s that extend the built-in themes.
//!
//! A skin file looks like:
//! ```ini
//! [skin]
//! description=my theme
//!
//! [core]
//! _default_=lightgray;blue
//! selected=black;cyan
//! marked=yellow;blue
//!
//! [filehighlight]
//! directory=white;
//! executable=brightgreen;
//! ```
//! Color values are `fg;bg` pairs. Each color may be an ANSI name
//! (`black`, `lightgray`, `brightgreen`, …), empty/`default` (terminal default),
//! `colorN` (0–255) or `#rrggbb` hex.

use std::path::PathBuf;

use ratatui::style::Color;

use crate::ui::theme::{IconThemeMode, ThemeId, ThemeSpec, THEMES};

/// The example skin shipped with the binary and seeded on first run.
const BUNDLED_MIDNIGHT_COMMANDER: &str = include_str!("../../assets/skins/midnight-commander.ini");

/// Returns the skins directory: `~/.config/sb/skins`.
fn skins_dir() -> PathBuf {
    crate::util::config::config_dir().join("skins")
}

/// Parses a single Midnight-Commander color token into a [`Color`].
///
/// Returns `None` for an empty token (meaning "leave the base value untouched").
/// `default` maps to the terminal default ([`Color::Reset`]).
fn parse_color(token: &str) -> Option<Color> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if lower == "default" {
        return Some(Color::Reset);
    }
    // #rrggbb hex
    if let Some(hex) = lower.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                let r = ((rgb >> 16) & 0xff) as u8;
                let g = ((rgb >> 8) & 0xff) as u8;
                let b = (rgb & 0xff) as u8;
                return Some(Color::Rgb(r, g, b));
            }
        }
        return None;
    }
    // colorN (0..=255) — MC also accepts `colorNN`
    if let Some(n) = lower.strip_prefix("color") {
        if let Ok(idx) = n.parse::<u8>() {
            return Some(Color::Indexed(idx));
        }
    }
    // Bare numeric index
    if let Ok(idx) = lower.parse::<u8>() {
        return Some(Color::Indexed(idx));
    }
    // Named ANSI colors (16-color palette).
    let named = match lower.as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "brown" | "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" | "lightgray" | "lightgrey" | "white" => {
            // MC's "white" on a 16-color palette is the dim white (gray);
            // the bright variants below are the true bright colors.
            if lower == "white" {
                Color::White
            } else {
                Color::Gray
            }
        }
        "darkgray" | "darkgrey" | "brightblack" => Color::DarkGray,
        "brightred" => Color::LightRed,
        "brightgreen" => Color::LightGreen,
        "brightyellow" => Color::LightYellow,
        "brightblue" => Color::LightBlue,
        "brightmagenta" => Color::LightMagenta,
        "brightcyan" => Color::LightCyan,
        "brightwhite" => Color::White,
        _ => return None,
    };
    Some(named)
}

/// Splits an MC `fg;bg` value into its foreground and background colors.
fn parse_pair(value: &str) -> (Option<Color>, Option<Color>) {
    let mut parts = value.splitn(2, ';');
    let fg = parts.next().map(parse_color).unwrap_or(None);
    let bg = parts.next().map(parse_color).unwrap_or(None);
    (fg, bg)
}

/// Parses skin file `text` into a [`ThemeSpec`], starting from the built-in
/// Original theme as a base so unspecified fields keep sensible defaults.
///
/// `fallback_name` (typically the file stem) is used when the skin omits a
/// `[skin] description`.
pub(crate) fn parse_skin(text: &str, fallback_name: &str) -> ThemeSpec {
    // Base off Original (index 0) so any field not set by the skin looks sane.
    let mut spec = THEMES[0];
    spec.id = ThemeId::original();
    // Custom skins are dark-ish by default for icon coloring.
    spec.icon_theme_mode = IconThemeMode::Dark;

    let mut description: Option<String> = None;
    let mut section = String::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();

        match section.as_str() {
            "skin" => {
                if key == "description" && !value.is_empty() {
                    description = Some(value.to_string());
                }
            }
            "core" => {
                let (fg, bg) = parse_pair(value);
                match key.as_str() {
                    "_default_" => {
                        if let Some(c) = fg {
                            spec.text_normal = c;
                            spec.icon_default_file = c;
                            spec.icon_os = c;
                        }
                        if let Some(c) = bg {
                            spec.bg_panel = c;
                        }
                    }
                    "selected" => {
                        if let Some(c) = fg {
                            spec.selected_fg = c;
                        }
                        if let Some(c) = bg {
                            spec.bg_selected = c;
                            spec.accent_primary = c;
                        }
                    }
                    "marked" => {
                        if let Some(c) = fg {
                            spec.marked_fg = c;
                            spec.warning = c;
                        }
                        if let Some(c) = bg {
                            spec.marked_bg = c;
                        }
                    }
                    "reverse" => {
                        if let Some(c) = bg {
                            spec.divider = c;
                        }
                    }
                    // sb extensions (not standard MC): muted text + frame/border color.
                    "dim" => {
                        if let Some(c) = fg {
                            spec.text_dim = c;
                        }
                    }
                    "border" => {
                        if let Some(c) = fg {
                            spec.border = c;
                        }
                    }
                    // gauge / input / markselect are accepted but not rendered yet.
                    _ => {}
                }
            }
            "filehighlight" => {
                let (fg, _bg) = parse_pair(value);
                if let Some(c) = fg {
                    match key.as_str() {
                        "directory" => spec.icon_default_dir = c,
                        "executable" => {
                            spec.text_executable = c;
                            spec.success = c;
                        }
                        "symlink" => spec.text_symlink = c,
                        "stalelink" => {
                            spec.text_stalelink = c;
                            spec.error = c;
                        }
                        "archive" => spec.text_archive = c,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let name = description.unwrap_or_else(|| fallback_name.to_string());
    // The registry lives for the whole process, so leaking the name string to
    // obtain a `&'static str` is acceptable (loaded once at startup).
    spec.name = Box::leak(name.into_boxed_str());
    spec
}

/// Writes the bundled example skin to `skins_dir()` if it is not already present.
fn seed_bundled_skin(dir: &std::path::Path) {
    let target = dir.join("midnight-commander.ini");
    if target.exists() {
        return;
    }
    if std::fs::create_dir_all(dir).is_ok() {
        let _ = std::fs::write(&target, BUNDLED_MIDNIGHT_COMMANDER);
    }
}

/// Loads all custom themes from `~/.config/sb/skins/*.ini`, sorted by name.
///
/// Seeds the bundled "midnight commander" example on first run. Unreadable or
/// unparseable files are skipped.
pub(crate) fn load_custom_themes() -> Vec<ThemeSpec> {
    let dir = skins_dir();
    seed_bundled_skin(&dir);

    let mut themes: Vec<ThemeSpec> = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return themes;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_ini = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ini"))
            .unwrap_or(false);
        if !is_ini {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom")
            .to_string();
        themes.push(parse_skin(&text, &stem));
    }
    themes.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    themes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_color_formats() {
        assert_eq!(parse_color(""), None);
        assert_eq!(parse_color("default"), Some(Color::Reset));
        assert_eq!(parse_color("blue"), Some(Color::Blue));
        assert_eq!(parse_color("brightgreen"), Some(Color::LightGreen));
        assert_eq!(parse_color("color123"), Some(Color::Indexed(123)));
        assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(parse_color("notacolor"), None);
    }

    #[test]
    fn parse_skin_maps_fields() {
        let spec = parse_skin(BUNDLED_MIDNIGHT_COMMANDER, "midnight-commander");
        assert_eq!(spec.name, "midnight commander");
        assert_eq!(spec.text_normal, Color::Gray); // lightgray
        assert_eq!(spec.bg_panel, Color::Blue);
        assert_eq!(spec.selected_fg, Color::Black);
        assert_eq!(spec.bg_selected, Color::Cyan);
        assert_eq!(spec.icon_default_dir, Color::White); // directory=white
        assert_eq!(spec.text_executable, Color::LightGreen); // brightgreen
        assert_eq!(spec.text_stalelink, Color::LightRed); // brightred
        assert_eq!(spec.text_archive, Color::Magenta);
        assert_eq!(spec.text_dim, Color::DarkGray); // dim=darkgray
        assert_eq!(spec.border, Color::Cyan); // border=cyan
    }
}
