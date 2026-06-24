use std::{collections::HashMap, fs, path::Path, str::FromStr, time::UNIX_EPOCH};

use crate::util::format::format_mtime;
use crate::ui::palette::Palette;
use crate::ui::theme::{theme_spec, IconThemeMode, ThemeId};
use devicons::{icon_for_file, File as DevFile, Theme};
use ratatui::prelude::*;
use crate::ui::icons::named_file_icon;
use crate::{ui, App, FilenameColorMode};

#[derive(Clone)]
pub(crate) struct EntryRenderCache {
    pub(crate) raw_name: String,
    pub(crate) icon_glyph: String,
    pub(crate) icon_style: Style,
    pub(crate) name_style: Style,
    pub(crate) perms_col: String,
    pub(crate) group_name: String,
    pub(crate) owner_name: String,
    pub(crate) size_col: String,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) date_col: String,
    pub(crate) modified_unix: Option<u64>,
}

/// Aggregate values for one file list that depend only on the entry set, not on
/// per-frame layout. Computed once whenever the panel's `entry_render_cache` is
/// rebuilt (directory change / sort / setting toggle) and read back during
/// rendering, so the render path no longer rescans/sorts every frame.
#[derive(Clone, Default)]
pub(crate) struct ListAggregates {
    /// `(min, max)` byte sizes across the list, for the size heat coloring.
    pub(crate) size_min_max: Option<(u64, u64)>,
    /// Per-timestamp rank in `[0,1]`, for the date heat coloring.
    pub(crate) date_rank_by_ts: HashMap<u64, f64>,
    /// Total bytes for the percent column — `Some` only when every entry has a
    /// known size (matches `panel_percent_total(.., true)`).
    pub(crate) percent_total: Option<u64>,
    /// Widest trimmed size column across the list (min 1).
    pub(crate) max_size_width: usize,
}

impl ListAggregates {
    pub(crate) fn from_cache(cache: &[EntryRenderCache]) -> Self {
        ListAggregates {
            size_min_max: ui::list_temperature::size_min_max_from_sizes(
                cache.iter().map(|entry| entry.size_bytes),
            ),
            date_rank_by_ts: ui::list_temperature::date_rank_map_from_unix(
                cache.iter().map(|entry| entry.modified_unix),
            ),
            percent_total: ui::list_metrics::panel_percent_total(cache, true),
            max_size_width: ui::list_metrics::panel_size_width(cache, true),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EntryRenderConfig {
    pub(crate) nerd_font_active: bool,
    pub(crate) show_icons: bool,
    pub(crate) theme_id: ThemeId,
    pub(crate) filename_color_mode: FilenameColorMode,
}

impl App {
    fn terminal_background_is_light() -> bool {
        std::env::var("COLORFGBG")
            .ok()
            .and_then(|value| {
                value
                    .split(';')
                    .filter_map(|part| part.trim().parse::<u8>().ok())
                    .next_back()
            })
            .map(|bg| bg >= 8)
            .unwrap_or(false)
    }

    fn icon_theme_for(theme_id: ThemeId) -> Theme {
        match theme_spec(theme_id).icon_theme_mode {
            IconThemeMode::Light => Theme::Light,
            IconThemeMode::Dark => Theme::Dark,
            IconThemeMode::Auto => {
                if Self::terminal_background_is_light() {
                    Theme::Light
                } else {
                    Theme::Dark
                }
            }
        }
    }

    pub(crate) fn icon_for_name(name: &str, is_dir: bool, show_icons: bool, nerd_font_active: bool, is_symlink: bool, theme_id: ThemeId) -> (String, Style) {
        let theme = theme_spec(theme_id);
        if !show_icons {
            return (String::new(), Style::default());
        }

        if is_symlink {
            return ("\u{f1177}".to_string(), Style::default().fg(theme.text_symlink));
        }

        if nerd_font_active {
            if is_dir {
                let dir_style = Style::default()
                    .fg(theme.icon_default_dir)
                    .add_modifier(Modifier::BOLD);
                if let Some((glyph, _)) = ui::icons::named_dir_icon(name) {
                    (glyph.to_string(), dir_style)
                } else {
                    ("\u{f024b}".to_string(), dir_style)
                }
            } else if name.trim().is_empty()
                || Path::new(name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            {
                // Draft/partial names in interactive prompts (e.g. empty line, '/')
                // can lack a valid filename component; avoid calling devicons in that case.
                ("\u{f15b}".to_string(), Style::default().fg(theme.text_normal))
            } else if Path::new(name)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("age"))
                .unwrap_or(false)
            {
                ("\u{f023}".to_string(), Style::default().fg(Palette::WARNING_ALT))
            } else if let Some((custom_icon, _)) = named_file_icon(name) {
                (custom_icon.to_string(), Style::default().fg(theme.icon_default_file))
            } else {
                let icon_theme = Self::icon_theme_for(theme_id);
                let data = icon_for_file(&DevFile::new(Path::new(name)), Some(icon_theme));
                let color = Color::from_str(data.color).unwrap_or(theme.icon_default_file);
                (data.icon.to_string(), Style::default().fg(color))
            }
        } else if is_dir {
            (
                "📁".to_string(),
                Style::default()
                    .fg(theme.icon_default_dir)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("📄".to_string(), Style::default().fg(theme.icon_default_file))
        }
    }

    pub(crate) fn icon_for_path(path: &Path, show_icons: bool, nerd_font_active: bool, is_symlink: bool, theme_id: ThemeId) -> (String, Style) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        Self::icon_for_name(name, path.is_dir(), show_icons, nerd_font_active, is_symlink, theme_id)
    }

    /// Lightweight, extension-only archive check used for filename coloring.
    /// (Avoids reading file signatures, unlike `archive_kind`.)
    fn is_archive_name(path: &Path) -> bool {
        const ARCHIVE_EXTENSIONS: &[&str] = &[
            "zip", "tar", "gz", "tgz", "bz2", "tbz", "tbz2", "xz", "txz", "zst", "tzst", "7z",
            "rar", "lz", "lzma", "lz4", "z", "cpio", "ar", "iso", "jar", "war", "deb", "rpm",
        ];
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ARCHIVE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn build_entry_render_cache(
        entry: &fs::DirEntry,
        config: EntryRenderConfig,
        uid_cache: &HashMap<u32, String>,
        gid_cache: &HashMap<u32, String>,
    ) -> EntryRenderCache {
        let path = entry.path();
        let meta = entry.metadata().ok();
        let is_hidden = crate::util::classify::is_hidden_entry(entry);
        let is_symlink = entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false);
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        // icon_data is still needed for name_style color on regular nerd-font files.
        let icon_data = if config.nerd_font_active && !is_symlink && !is_dir {
            Some(icon_for_file(
                &DevFile::new(&path),
                Some(Self::icon_theme_for(config.theme_id)),
            ))
        } else {
            None
        };

        let (icon_glyph, icon_style) = Self::icon_for_path(
            &path,
            config.show_icons,
            config.nerd_font_active,
            is_symlink,
            config.theme_id,
        );

        let theme = theme_spec(config.theme_id);
        let is_archive = !is_dir && !is_symlink && Self::is_archive_name(&path);
        let is_age = !is_dir && Self::is_age_protected_file(&path);
        // Whether the entry is executable (Unix permission bits). Computed here so
        // the filename-color mode below can preserve this status color.
        let is_exec = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                !is_dir
                    && !is_symlink
                    && meta
                        .as_ref()
                        .map(|m| m.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false)
            }
            #[cfg(not(unix))]
            {
                false
            }
        };
        let mut name_style = if is_dir {
            Style::default()
                .fg(theme.icon_default_dir)
                .add_modifier(Modifier::BOLD)
        } else if is_age {
            Style::default().fg(Palette::WARNING_ALT)
        } else if is_archive {
            Style::default().fg(theme.text_archive)
        } else {
            let file_color = icon_data
                .as_ref()
                .and_then(|i| Color::from_str(i.color).ok())
                .unwrap_or(theme.text_normal);
            Style::default().fg(file_color)
        };

        if is_exec {
            name_style = Style::default().fg(theme.text_executable);
        }

        // Symlinks take their own color; broken (stale) links are flagged.
        if is_symlink {
            name_style = if path.exists() {
                Style::default().fg(theme.text_symlink)
            } else {
                Style::default().fg(theme.text_stalelink)
            };
        }

        // Apply the filename-color mode (folders are never affected; only the
        // name color changes — icon colors are left untouched). "White" forces
        // all file names to the theme's normal text color; "Less" does the same
        // but keeps status colors (executable, symlink, archive, age) intact.
        if !is_dir {
            match config.filename_color_mode {
                FilenameColorMode::Full => {}
                FilenameColorMode::Less => {
                    if !(is_archive || is_symlink || is_exec || is_age) {
                        name_style = name_style.fg(theme.text_normal);
                    }
                }
                FilenameColorMode::White => {
                    name_style = name_style.fg(theme.text_normal);
                }
            }
        }

        if is_hidden {
            name_style = name_style.add_modifier(Modifier::DIM);
        }

        let perms_width = 11usize;
        let size_width = 6usize;
        let date_width = 16usize;
        let perms = meta
            .as_ref()
            .map(App::parse_permissions)
            .unwrap_or_else(|| "----------".to_string());
        let owner = meta
            .as_ref()
            .map(|m| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let uid = m.uid();
                    uid_cache
                        .get(&uid)
                        .cloned()
                        .unwrap_or_else(|| uid.to_string())
                }
                #[cfg(not(unix))]
                {
                    "-".to_string()
                }
            })
            .unwrap_or_else(|| "-".to_string());
        let group = meta
            .as_ref()
            .map(|m| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let gid = m.gid();
                    gid_cache
                        .get(&gid)
                        .cloned()
                        .unwrap_or_else(|| gid.to_string())
                }
                #[cfg(not(unix))]
                {
                    "-".to_string()
                }
            })
            .unwrap_or_else(|| "-".to_string());
        let perms_col = format!("{:<width$}", perms, width = perms_width);
        let size_bytes = meta
            .as_ref()
            .and_then(|m| if m.is_dir() { None } else { Some(Self::display_leaf_size(m)) });
        let size = size_bytes
            .map(App::format_size)
            .unwrap_or_else(|| "-".to_string());
        let size_col = format!("{:>width$}", size, width = size_width);
        let date = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(format_mtime)
            .unwrap_or_default();
        let date_col = format!("{:>width$}", date, width = date_width);
        let modified_unix = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        EntryRenderCache {
            raw_name: crate::util::classify::entry_name(entry),
            icon_glyph,
            icon_style,
            name_style,
            perms_col,
            group_name: group,
            owner_name: owner,
            size_col,
            size_bytes,
            date_col,
            modified_unix,
        }
    }

    pub(crate) fn refresh_meta_identity_widths(&mut self) {
        let mut group_w = 1usize;
        let mut owner_w = 1usize;
        for entry in &self.left.entry_render_cache {
            group_w = group_w.max(entry.group_name.chars().count());
            owner_w = owner_w.max(entry.owner_name.chars().count());
        }
        self.meta_group_width = group_w.min(16);
        self.meta_owner_width = owner_w.min(20);
    }
}
