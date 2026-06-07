use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeId {
    Original,
    Nord,
    Solarized,
    Gruvbox,
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
    pub(crate) bg_panel: Color,
    pub(crate) divider: Color,
    pub(crate) icon_default_file: Color,
    pub(crate) icon_default_dir: Color,
    pub(crate) icon_os: Color,
}

pub(crate) const THEMES: [ThemeSpec; 4] = [
    ThemeSpec {
        id: ThemeId::Original,
        name: "original",
        text_normal: Color::Reset,
        accent_primary: Color::Rgb(100, 160, 240),
        success: Color::Rgb(100, 220, 120),
        warning: Color::Rgb(245, 200, 90),
        error: Color::Rgb(220, 80, 80),
        bg_selected: Color::DarkGray,
        bg_panel: Color::Reset,
        divider: Color::Rgb(80, 200, 180),
        icon_default_file: Color::Reset,
        icon_default_dir: Color::Rgb(100, 160, 240),
        icon_os: Color::Reset,
    },
    ThemeSpec {
        id: ThemeId::Nord,
        name: "nord",
        text_normal: Color::Rgb(216, 222, 233),
        accent_primary: Color::Rgb(129, 161, 193),
        success: Color::Rgb(163, 190, 140),
        warning: Color::Rgb(235, 203, 139),
        error: Color::Rgb(191, 97, 106),
        bg_selected: Color::Rgb(59, 66, 82),
        bg_panel: Color::Rgb(46, 52, 64),
        divider: Color::Rgb(136, 192, 208),
        icon_default_file: Color::Rgb(216, 222, 233),
        icon_default_dir: Color::Rgb(129, 161, 193),
        icon_os: Color::Rgb(143, 188, 187),
    },
    ThemeSpec {
        id: ThemeId::Solarized,
        name: "solarized",
        text_normal: Color::Rgb(131, 148, 150),
        accent_primary: Color::Rgb(38, 139, 210),
        success: Color::Rgb(133, 153, 0),
        warning: Color::Rgb(181, 137, 0),
        error: Color::Rgb(220, 50, 47),
        bg_selected: Color::Rgb(7, 54, 66),
        bg_panel: Color::Rgb(0, 43, 54),
        divider: Color::Rgb(42, 161, 152),
        icon_default_file: Color::Rgb(147, 161, 161),
        icon_default_dir: Color::Rgb(38, 139, 210),
        icon_os: Color::Rgb(42, 161, 152),
    },
    ThemeSpec {
        id: ThemeId::Gruvbox,
        name: "gruvbox",
        text_normal: Color::Rgb(235, 219, 178),
        accent_primary: Color::Rgb(131, 165, 152),
        success: Color::Rgb(184, 187, 38),
        warning: Color::Rgb(250, 189, 47),
        error: Color::Rgb(251, 73, 52),
        bg_selected: Color::Rgb(60, 56, 54),
        bg_panel: Color::Rgb(40, 40, 40),
        divider: Color::Rgb(142, 192, 124),
        icon_default_file: Color::Rgb(235, 219, 178),
        icon_default_dir: Color::Rgb(131, 165, 152),
        icon_os: Color::Rgb(215, 153, 33),
    },
];

pub(crate) fn theme_by_name(name: &str) -> ThemeId {
    match name.to_ascii_lowercase().as_str() {
        "nord" => ThemeId::Nord,
        "solarized" => ThemeId::Solarized,
        "gruvbox" => ThemeId::Gruvbox,
        _ => ThemeId::Original,
    }
}

pub(crate) fn theme_name(id: ThemeId) -> &'static str {
    match id {
        ThemeId::Original => "original",
        ThemeId::Nord => "nord",
        ThemeId::Solarized => "solarized",
        ThemeId::Gruvbox => "gruvbox",
    }
}

pub(crate) fn theme_spec(id: ThemeId) -> &'static ThemeSpec {
    match id {
        ThemeId::Original => &THEMES[0],
        ThemeId::Nord => &THEMES[1],
        ThemeId::Solarized => &THEMES[2],
        ThemeId::Gruvbox => &THEMES[3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_lookup_defaults_to_original() {
        assert_eq!(theme_by_name("does-not-exist"), ThemeId::Original);
    }

    #[test]
    fn theme_roundtrip_name() {
        let id = theme_by_name("nord");
        assert_eq!(theme_name(id), "nord");
    }
}