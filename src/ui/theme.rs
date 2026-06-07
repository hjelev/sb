use ratatui::style::Color;
use std::sync::OnceLock;

use crate::ui::palette::Palette;

/// Which icon color set (light/dark) a theme prefers.
///
/// `Auto` defers to terminal background detection (used by the Original theme).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconThemeMode {
    Dark,
    Light,
    Auto,
}

/// Identifier for a theme: an index into the runtime theme [`registry`].
///
/// Index 0 is always the built-in Original theme. Indices `0..4` are the
/// built-in themes; any further indices are custom themes loaded from
/// `~/.config/sb/skins/*.ini` at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ThemeId(pub(crate) usize);

impl ThemeId {
    /// The default built-in theme (Original), always present at index 0.
    pub(crate) fn original() -> Self {
        ThemeId(0)
    }
}

impl Default for ThemeId {
    fn default() -> Self {
        ThemeId::original()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ThemeSpec {
    pub(crate) id: ThemeId,
    pub(crate) name: &'static str,
    pub(crate) text_normal: Color,
    pub(crate) accent_primary: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
    pub(crate) bg_selected: Color,
    /// Foreground for the selected/cursor row (MC `selected` fg).
    pub(crate) selected_fg: Color,
    pub(crate) bg_panel: Color,
    pub(crate) divider: Color,
    /// Secondary/muted text (shortcut descriptions, notes, hints, inactive labels).
    pub(crate) text_dim: Color,
    /// Subtle structural lines (panel frame borders, scrollbar track, separators).
    pub(crate) border: Color,
    pub(crate) icon_default_file: Color,
    pub(crate) icon_default_dir: Color,
    pub(crate) icon_os: Color,
    /// Filename color for executables (MC `[filehighlight] executable`).
    pub(crate) text_executable: Color,
    /// Filename color for symlinks (MC `[filehighlight] symlink`).
    pub(crate) text_symlink: Color,
    /// Filename color for broken/stale symlinks (MC `[filehighlight] stalelink`).
    pub(crate) text_stalelink: Color,
    /// Filename color for archives (MC `[filehighlight] archive`).
    pub(crate) text_archive: Color,
    /// Foreground for marked/tagged files (MC `marked`).
    pub(crate) marked_fg: Color,
    /// Background for marked/tagged files (MC `marked`).
    pub(crate) marked_bg: Color,
    /// Preferred icon color set (light/dark/auto).
    pub(crate) icon_theme_mode: IconThemeMode,
}

/// The built-in themes. These seed the runtime [`registry`]; their `id` fields
/// are overwritten with their registry index when the registry is built.
pub(crate) const THEMES: [ThemeSpec; 4] = [
    ThemeSpec {
        id: ThemeId(0),
        name: "original",
        text_normal: Color::Reset,
        accent_primary: Color::Rgb(100, 160, 240),
        success: Color::Rgb(100, 220, 120),
        warning: Color::Rgb(245, 200, 90),
        error: Color::Rgb(220, 80, 80),
        bg_selected: Color::DarkGray,
        selected_fg: Color::Reset,
        bg_panel: Color::Reset,
        divider: Color::Rgb(80, 200, 180),
        text_dim: Color::Rgb(150, 150, 150),
        border: Color::DarkGray,
        icon_default_file: Color::Reset,
        icon_default_dir: Color::Rgb(100, 160, 240),
        icon_os: Color::Reset,
        text_executable: Palette::SUCCESS_ALT,
        text_symlink: Palette::SYMLINK,
        text_stalelink: Color::Rgb(220, 80, 80),
        text_archive: Color::Rgb(200, 120, 220),
        marked_fg: Color::Rgb(245, 200, 90),
        marked_bg: Palette::BG_MARKED,
        icon_theme_mode: IconThemeMode::Auto,
    },
    ThemeSpec {
        id: ThemeId(1),
        name: "nord",
        text_normal: Color::Rgb(216, 222, 233),
        accent_primary: Color::Rgb(129, 161, 193),
        success: Color::Rgb(163, 190, 140),
        warning: Color::Rgb(235, 203, 139),
        error: Color::Rgb(191, 97, 106),
        bg_selected: Color::Rgb(59, 66, 82),
        selected_fg: Color::Rgb(236, 239, 244),
        bg_panel: Color::Rgb(46, 52, 64),
        divider: Color::Rgb(136, 192, 208),
        text_dim: Color::Rgb(110, 120, 140),
        border: Color::Rgb(76, 86, 106),
        icon_default_file: Color::Rgb(216, 222, 233),
        icon_default_dir: Color::Rgb(129, 161, 193),
        icon_os: Color::Rgb(143, 188, 187),
        text_executable: Color::Rgb(163, 190, 140),
        text_symlink: Color::Rgb(136, 192, 208),
        text_stalelink: Color::Rgb(191, 97, 106),
        text_archive: Color::Rgb(180, 142, 173),
        marked_fg: Color::Rgb(235, 203, 139),
        marked_bg: Color::Rgb(67, 76, 94),
        icon_theme_mode: IconThemeMode::Dark,
    },
    ThemeSpec {
        id: ThemeId(2),
        name: "solarized",
        text_normal: Color::Rgb(131, 148, 150),
        accent_primary: Color::Rgb(38, 139, 210),
        success: Color::Rgb(133, 153, 0),
        warning: Color::Rgb(181, 137, 0),
        error: Color::Rgb(220, 50, 47),
        bg_selected: Color::Rgb(7, 54, 66),
        selected_fg: Color::Rgb(147, 161, 161),
        bg_panel: Color::Rgb(0, 43, 54),
        divider: Color::Rgb(42, 161, 152),
        text_dim: Color::Rgb(88, 110, 117),
        border: Color::Rgb(88, 110, 117),
        icon_default_file: Color::Rgb(147, 161, 161),
        icon_default_dir: Color::Rgb(38, 139, 210),
        icon_os: Color::Rgb(42, 161, 152),
        text_executable: Color::Rgb(133, 153, 0),
        text_symlink: Color::Rgb(42, 161, 152),
        text_stalelink: Color::Rgb(220, 50, 47),
        text_archive: Color::Rgb(211, 54, 130),
        marked_fg: Color::Rgb(181, 137, 0),
        marked_bg: Color::Rgb(7, 54, 66),
        icon_theme_mode: IconThemeMode::Light,
    },
    ThemeSpec {
        id: ThemeId(3),
        name: "gruvbox",
        text_normal: Color::Rgb(235, 219, 178),
        accent_primary: Color::Rgb(131, 165, 152),
        success: Color::Rgb(184, 187, 38),
        warning: Color::Rgb(250, 189, 47),
        error: Color::Rgb(251, 73, 52),
        bg_selected: Color::Rgb(60, 56, 54),
        selected_fg: Color::Rgb(251, 241, 199),
        bg_panel: Color::Rgb(40, 40, 40),
        divider: Color::Rgb(142, 192, 124),
        text_dim: Color::Rgb(146, 131, 116),
        border: Color::Rgb(80, 73, 69),
        icon_default_file: Color::Rgb(235, 219, 178),
        icon_default_dir: Color::Rgb(131, 165, 152),
        icon_os: Color::Rgb(215, 153, 33),
        text_executable: Color::Rgb(184, 187, 38),
        text_symlink: Color::Rgb(142, 192, 124),
        text_stalelink: Color::Rgb(251, 73, 52),
        text_archive: Color::Rgb(211, 134, 155),
        marked_fg: Color::Rgb(250, 189, 47),
        marked_bg: Color::Rgb(60, 56, 54),
        icon_theme_mode: IconThemeMode::Dark,
    },
];

/// Process-lifetime list of all available themes (built-ins + loaded custom skins).
static REGISTRY: OnceLock<Vec<ThemeSpec>> = OnceLock::new();

/// Builds the theme registry: the four built-ins followed by any custom skins
/// found in `~/.config/sb/skins/`. Each entry's `id` is set to its index.
fn load_all_themes() -> Vec<ThemeSpec> {
    let mut themes: Vec<ThemeSpec> = THEMES.to_vec();
    themes.extend(crate::ui::skin::load_custom_themes());
    for (idx, theme) in themes.iter_mut().enumerate() {
        theme.id = ThemeId(idx);
    }
    themes
}

/// Returns the full theme registry (built-ins + custom skins), loaded once.
pub(crate) fn themes() -> &'static [ThemeSpec] {
    REGISTRY.get_or_init(load_all_themes)
}

pub(crate) fn theme_by_name(name: &str) -> ThemeId {
    let target = name.trim().to_ascii_lowercase();
    themes()
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(&target))
        .map(|t| t.id)
        .unwrap_or_else(ThemeId::original)
}

pub(crate) fn theme_name(id: ThemeId) -> &'static str {
    theme_spec(id).name
}

pub(crate) fn theme_spec(id: ThemeId) -> &'static ThemeSpec {
    let registry = themes();
    registry.get(id.0).unwrap_or(&registry[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_lookup_defaults_to_original() {
        assert_eq!(theme_by_name("does-not-exist"), ThemeId::original());
    }

    #[test]
    fn theme_roundtrip_name() {
        let id = theme_by_name("nord");
        assert_eq!(theme_name(id), "nord");
    }

    #[test]
    fn builtins_present_and_indexed() {
        let registry = themes();
        assert!(registry.len() >= 4);
        assert_eq!(registry[0].name, "original");
        for (idx, theme) in registry.iter().enumerate() {
            assert_eq!(theme.id.0, idx);
        }
    }
}
