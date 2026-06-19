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
/// Index 0 is always the built-in Original theme. Indices `0..11` are the
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
    /// Git added / new-file indicator.
    pub(crate) git_added: Color,
    /// Git modified / renamed-file indicator.
    pub(crate) git_modified: Color,
    /// Git deleted / removed-file indicator.
    pub(crate) git_deleted: Color,
    /// Inactive-panel row background in dual-panel mode.
    pub(crate) bg_inactive_panel: Color,
    /// Keyboard shortcut key labels in overlay panels (e.g. "↑↓", "Enter").
    pub(crate) key_label: Color,
    /// Section / heading text in overlay panels (help, integrations, sort).
    pub(crate) overlay_section: Color,
}

/// The built-in themes. These seed the runtime [`registry`]; their `id` fields
/// are overwritten with their registry index when the registry is built.
pub(crate) const THEMES: [ThemeSpec; 11] = [
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
        git_added: Color::Rgb(150, 220, 150),
        git_modified: Color::Rgb(120, 200, 255),
        git_deleted: Color::Rgb(255, 120, 120),
        bg_inactive_panel: Color::Rgb(38, 38, 45),
        key_label: Color::Rgb(255, 220, 120),
        overlay_section: Color::Rgb(120, 200, 255),
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
        git_added: Color::Rgb(163, 190, 140),
        git_modified: Color::Rgb(129, 161, 193),
        git_deleted: Color::Rgb(191, 97, 106),
        bg_inactive_panel: Color::Rgb(36, 41, 52),
        key_label: Color::Rgb(235, 203, 139),
        overlay_section: Color::Rgb(129, 161, 193),
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
        git_added: Color::Rgb(133, 153, 0),
        git_modified: Color::Rgb(38, 139, 210),
        git_deleted: Color::Rgb(220, 50, 47),
        bg_inactive_panel: Color::Rgb(0, 28, 36),
        key_label: Color::Rgb(181, 137, 0),
        overlay_section: Color::Rgb(38, 139, 210),
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
        git_added: Color::Rgb(184, 187, 38),
        git_modified: Color::Rgb(131, 165, 152),
        git_deleted: Color::Rgb(251, 73, 52),
        bg_inactive_panel: Color::Rgb(29, 28, 28),
        key_label: Color::Rgb(250, 189, 47),
        overlay_section: Color::Rgb(131, 165, 152),
    },
    // Hercules monochrome monitor: green phosphor on black.
    ThemeSpec {
        id: ThemeId(4),
        name: "aiberto",
        text_normal: Color::Rgb(51, 255, 102),
        accent_primary: Color::Rgb(120, 255, 150),
        success: Color::Rgb(80, 255, 120),
        warning: Color::Rgb(180, 255, 120),
        error: Color::Rgb(255, 90, 60),
        bg_selected: Color::Rgb(0, 80, 30),
        selected_fg: Color::Rgb(180, 255, 200),
        bg_panel: Color::Rgb(0, 0, 0),
        divider: Color::Rgb(120, 255, 150),
        text_dim: Color::Rgb(0, 130, 60),
        border: Color::Rgb(0, 90, 45),
        icon_default_file: Color::Rgb(51, 255, 102),
        icon_default_dir: Color::Rgb(120, 255, 150),
        icon_os: Color::Rgb(51, 255, 102),
        text_executable: Color::Rgb(80, 255, 120),
        text_symlink: Color::Rgb(150, 255, 190),
        text_stalelink: Color::Rgb(255, 90, 60),
        text_archive: Color::Rgb(0, 200, 120),
        marked_fg: Color::Rgb(180, 255, 120),
        marked_bg: Color::Rgb(0, 60, 25),
        icon_theme_mode: IconThemeMode::Dark,
        git_added: Color::Rgb(80, 255, 120),
        git_modified: Color::Rgb(150, 255, 190),
        git_deleted: Color::Rgb(255, 90, 60),
        bg_inactive_panel: Color::Rgb(5, 15, 5),
        key_label: Color::Rgb(180, 255, 120),
        overlay_section: Color::Rgb(120, 255, 150),
    },
    ThemeSpec {
        id: ThemeId(5),
        name: "dracula",
        text_normal: Color::Rgb(248, 248, 242),
        accent_primary: Color::Rgb(189, 147, 249),
        success: Color::Rgb(80, 250, 123),
        warning: Color::Rgb(241, 250, 140),
        error: Color::Rgb(255, 85, 85),
        bg_selected: Color::Rgb(68, 71, 90),
        selected_fg: Color::Rgb(248, 248, 242),
        bg_panel: Color::Rgb(40, 42, 54),
        divider: Color::Rgb(139, 233, 253),
        text_dim: Color::Rgb(98, 114, 164),
        border: Color::Rgb(98, 114, 164),
        icon_default_file: Color::Rgb(248, 248, 242),
        icon_default_dir: Color::Rgb(189, 147, 249),
        icon_os: Color::Rgb(139, 233, 253),
        text_executable: Color::Rgb(80, 250, 123),
        text_symlink: Color::Rgb(139, 233, 253),
        text_stalelink: Color::Rgb(255, 85, 85),
        text_archive: Color::Rgb(255, 121, 198),
        marked_fg: Color::Rgb(241, 250, 140),
        marked_bg: Color::Rgb(68, 71, 90),
        icon_theme_mode: IconThemeMode::Dark,
        git_added: Color::Rgb(80, 250, 123),
        git_modified: Color::Rgb(139, 233, 253),
        git_deleted: Color::Rgb(255, 85, 85),
        bg_inactive_panel: Color::Rgb(33, 34, 44),
        key_label: Color::Rgb(241, 250, 140),
        overlay_section: Color::Rgb(189, 147, 249),
    },
    ThemeSpec {
        id: ThemeId(6),
        name: "rose-pine",
        text_normal: Color::Rgb(224, 222, 244),
        accent_primary: Color::Rgb(49, 116, 143),
        success: Color::Rgb(156, 207, 216),
        warning: Color::Rgb(246, 193, 119),
        error: Color::Rgb(235, 111, 146),
        bg_selected: Color::Rgb(38, 35, 58),
        selected_fg: Color::Rgb(224, 222, 244),
        bg_panel: Color::Rgb(25, 23, 36),
        divider: Color::Rgb(156, 207, 216),
        text_dim: Color::Rgb(110, 106, 134),
        border: Color::Rgb(64, 61, 82),
        icon_default_file: Color::Rgb(224, 222, 244),
        icon_default_dir: Color::Rgb(49, 116, 143),
        icon_os: Color::Rgb(156, 207, 216),
        text_executable: Color::Rgb(156, 207, 216),
        text_symlink: Color::Rgb(156, 207, 216),
        text_stalelink: Color::Rgb(235, 111, 146),
        text_archive: Color::Rgb(196, 167, 231),
        marked_fg: Color::Rgb(246, 193, 119),
        marked_bg: Color::Rgb(38, 35, 58),
        icon_theme_mode: IconThemeMode::Dark,
        git_added: Color::Rgb(156, 207, 216),
        git_modified: Color::Rgb(49, 116, 143),
        git_deleted: Color::Rgb(235, 111, 146),
        bg_inactive_panel: Color::Rgb(31, 29, 46),
        key_label: Color::Rgb(246, 193, 119),
        overlay_section: Color::Rgb(196, 167, 231),
    },
    ThemeSpec {
        id: ThemeId(7),
        name: "everforest",
        text_normal: Color::Rgb(211, 198, 170),
        accent_primary: Color::Rgb(127, 187, 179),
        success: Color::Rgb(167, 192, 128),
        warning: Color::Rgb(219, 188, 127),
        error: Color::Rgb(230, 126, 128),
        bg_selected: Color::Rgb(71, 82, 88),
        selected_fg: Color::Rgb(211, 198, 170),
        bg_panel: Color::Rgb(45, 53, 59),
        divider: Color::Rgb(131, 192, 146),
        text_dim: Color::Rgb(133, 146, 137),
        border: Color::Rgb(133, 146, 137),
        icon_default_file: Color::Rgb(211, 198, 170),
        icon_default_dir: Color::Rgb(127, 187, 179),
        icon_os: Color::Rgb(131, 192, 146),
        text_executable: Color::Rgb(167, 192, 128),
        text_symlink: Color::Rgb(131, 192, 146),
        text_stalelink: Color::Rgb(230, 126, 128),
        text_archive: Color::Rgb(214, 153, 182),
        marked_fg: Color::Rgb(219, 188, 127),
        marked_bg: Color::Rgb(71, 82, 88),
        icon_theme_mode: IconThemeMode::Dark,
        git_added: Color::Rgb(167, 192, 128),
        git_modified: Color::Rgb(127, 187, 179),
        git_deleted: Color::Rgb(230, 126, 128),
        bg_inactive_panel: Color::Rgb(35, 42, 46),
        key_label: Color::Rgb(219, 188, 127),
        overlay_section: Color::Rgb(127, 187, 179),
    },
    ThemeSpec {
        id: ThemeId(8),
        name: "kanagawa",
        text_normal: Color::Rgb(220, 215, 186),
        accent_primary: Color::Rgb(126, 156, 216),
        success: Color::Rgb(152, 187, 108),
        warning: Color::Rgb(230, 195, 132),
        error: Color::Rgb(255, 93, 98),
        bg_selected: Color::Rgb(45, 79, 103),
        selected_fg: Color::Rgb(220, 215, 186),
        bg_panel: Color::Rgb(31, 31, 40),
        divider: Color::Rgb(122, 168, 159),
        text_dim: Color::Rgb(114, 113, 105),
        border: Color::Rgb(84, 84, 109),
        icon_default_file: Color::Rgb(220, 215, 186),
        icon_default_dir: Color::Rgb(126, 156, 216),
        icon_os: Color::Rgb(122, 168, 159),
        text_executable: Color::Rgb(152, 187, 108),
        text_symlink: Color::Rgb(127, 180, 202),
        text_stalelink: Color::Rgb(255, 93, 98),
        text_archive: Color::Rgb(149, 127, 184),
        marked_fg: Color::Rgb(230, 195, 132),
        marked_bg: Color::Rgb(45, 79, 103),
        icon_theme_mode: IconThemeMode::Dark,
        git_added: Color::Rgb(152, 187, 108),
        git_modified: Color::Rgb(126, 156, 216),
        git_deleted: Color::Rgb(255, 93, 98),
        bg_inactive_panel: Color::Rgb(22, 22, 29),
        key_label: Color::Rgb(230, 195, 132),
        overlay_section: Color::Rgb(126, 156, 216),
    },
    ThemeSpec {
        id: ThemeId(9),
        name: "onedark",
        text_normal: Color::Rgb(171, 178, 191),
        accent_primary: Color::Rgb(97, 175, 239),
        success: Color::Rgb(152, 195, 121),
        warning: Color::Rgb(229, 192, 123),
        error: Color::Rgb(224, 108, 117),
        bg_selected: Color::Rgb(62, 68, 81),
        selected_fg: Color::Rgb(171, 178, 191),
        bg_panel: Color::Rgb(40, 44, 52),
        divider: Color::Rgb(86, 182, 194),
        text_dim: Color::Rgb(92, 99, 112),
        border: Color::Rgb(62, 68, 81),
        icon_default_file: Color::Rgb(171, 178, 191),
        icon_default_dir: Color::Rgb(97, 175, 239),
        icon_os: Color::Rgb(86, 182, 194),
        text_executable: Color::Rgb(152, 195, 121),
        text_symlink: Color::Rgb(86, 182, 194),
        text_stalelink: Color::Rgb(224, 108, 117),
        text_archive: Color::Rgb(198, 120, 221),
        marked_fg: Color::Rgb(229, 192, 123),
        marked_bg: Color::Rgb(62, 68, 81),
        icon_theme_mode: IconThemeMode::Dark,
        git_added: Color::Rgb(152, 195, 121),
        git_modified: Color::Rgb(97, 175, 239),
        git_deleted: Color::Rgb(224, 108, 117),
        bg_inactive_panel: Color::Rgb(33, 37, 43),
        key_label: Color::Rgb(229, 192, 123),
        overlay_section: Color::Rgb(97, 175, 239),
    },
    // Banana: amber/orange monochrome on black, sibling of aiberto.
    ThemeSpec {
        id: ThemeId(10),
        name: "bannana",
        text_normal: Color::Rgb(255, 204, 51),
        accent_primary: Color::Rgb(255, 165, 60),
        success: Color::Rgb(255, 220, 90),
        warning: Color::Rgb(255, 235, 120),
        error: Color::Rgb(255, 90, 60),
        bg_selected: Color::Rgb(90, 50, 0),
        selected_fg: Color::Rgb(255, 235, 180),
        bg_panel: Color::Rgb(0, 0, 0),
        divider: Color::Rgb(255, 165, 60),
        text_dim: Color::Rgb(150, 100, 20),
        border: Color::Rgb(110, 70, 10),
        icon_default_file: Color::Rgb(255, 204, 51),
        icon_default_dir: Color::Rgb(255, 165, 60),
        icon_os: Color::Rgb(255, 204, 51),
        text_executable: Color::Rgb(255, 220, 90),
        text_symlink: Color::Rgb(255, 215, 130),
        text_stalelink: Color::Rgb(255, 90, 60),
        text_archive: Color::Rgb(230, 140, 40),
        marked_fg: Color::Rgb(255, 235, 120),
        marked_bg: Color::Rgb(70, 40, 0),
        icon_theme_mode: IconThemeMode::Dark,
        git_added: Color::Rgb(255, 220, 90),
        git_modified: Color::Rgb(255, 215, 130),
        git_deleted: Color::Rgb(255, 90, 60),
        bg_inactive_panel: Color::Rgb(15, 10, 0),
        key_label: Color::Rgb(255, 235, 120),
        overlay_section: Color::Rgb(255, 165, 60),
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
