use crate::integration::rows::IntegrationRow;
use crate::ui::theme::{theme_spec, themes, ThemeId};
use crate::SortMode;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

const PANEL_TABS: &[(&str, u8)] = &[
    (" Help ", 0),
    (" Search ", 1),
    (" Bookmarks ", 2),
    (" Remote Mounts ", 3),
    (" Sorting ", 4),
    (" Integrations ", 5),
    (" Themes ", 6),
];

// Scroll indicators shown when the tab bar is wider than the available space.
const TAB_MORE_LEFT: &str = "‹";
const TAB_MORE_RIGHT: &str = "›";

/// Rendered width of a single tab label (its full padded text).
fn tab_label_width(index: usize) -> usize {
    PANEL_TABS[index].0.chars().count()
}

/// Decide which contiguous run of tabs is visible for the given `active` tab and
/// available title `avail` width. Returns `(lo, hi, more_left, more_right)` where
/// `lo..=hi` is the visible range (always including `active`) and the booleans
/// indicate hidden tabs beyond each edge (drawn as `‹` / `›`). When everything
/// fits, the full range is returned and no indicators are shown.
fn visible_tab_window(active: usize, avail: usize) -> (usize, usize, bool, bool) {
    let n = PANEL_TABS.len();
    let active = active.min(n - 1);

    let full: usize = (0..n).map(tab_label_width).sum::<usize>() + n.saturating_sub(1);
    if full <= avail {
        return (0, n - 1, false, false);
    }

    // Brute-force the widest contiguous window containing `active` that fits.
    let mut best = (active, active);
    let mut best_count = 0usize;
    for lo in 0..=active {
        for hi in active..n {
            let mut w: usize = (lo..=hi).map(tab_label_width).sum::<usize>() + (hi - lo);
            if lo > 0 {
                w += TAB_MORE_LEFT.chars().count();
            }
            if hi < n - 1 {
                w += TAB_MORE_RIGHT.chars().count();
            }
            if w <= avail && (hi - lo + 1) > best_count {
                best_count = hi - lo + 1;
                best = (lo, hi);
            }
        }
    }
    let (lo, hi) = best;
    (lo, hi, lo > 0, hi < n - 1)
}

pub fn panel_tab_bar_line(
    active: u8,
    theme_id: ThemeId,
    nerd_font: bool,
    avail_width: u16,
) -> Line<'static> {
    let spec = theme_spec(theme_id);
    let inactive_style = Style::default().fg(spec.text_dim);
    let sep_style = Style::default().fg(spec.divider);
    let indicator_style = Style::default()
        .fg(spec.divider)
        .add_modifier(Modifier::BOLD);
    let pill_bg = spec.divider;
    // Dividers are bright across all themes, so a dark foreground keeps the
    // active tab label readable on the pill.
    let pill_text = Style::default()
        .fg(Color::Rgb(20, 20, 20))
        .bg(pill_bg)
        .add_modifier(Modifier::BOLD);

    let (lo, hi, more_left, more_right) =
        visible_tab_window(active as usize, avail_width as usize);

    let mut spans: Vec<Span<'static>> = Vec::new();
    if more_left {
        spans.push(Span::styled(TAB_MORE_LEFT, indicator_style));
    }
    for i in lo..=hi {
        let (label, idx) = PANEL_TABS[i];
        if i > lo {
            spans.push(Span::styled("─", sep_style));
        }
        if idx == active {
            // Active tab: a rounded pill with the selected background. The cap
            // glyphs replace the label's existing padding spaces so the rendered
            // width is unchanged — keeping `panel_tab_hit_test` aligned.
            if nerd_font {
                let cap_style = Style::default().fg(pill_bg);
                spans.push(Span::styled(PILL_LEFT_CAP, cap_style));
                spans.push(Span::styled(label.trim().to_string(), pill_text));
                spans.push(Span::styled(PILL_RIGHT_CAP, cap_style));
            } else {
                spans.push(Span::styled(label, pill_text));
            }
        } else {
            spans.push(Span::styled(label, inactive_style));
        }
    }
    if more_right {
        spans.push(Span::styled(TAB_MORE_RIGHT, indicator_style));
    }
    Line::from(spans)
}

/// Render a rounded overlay block with title and a close `x` button.
/// Returns the inner `Rect` for content placement.
fn render_overlay_block(
    f: &mut Frame,
    area: Rect,
    panel_tab: u8,
    theme_id: ThemeId,
    nerd_font: bool,
) -> Rect {
    use crate::ui::theme::theme_spec;
    let spec = theme_spec(theme_id);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(panel_tab_bar_line(panel_tab, theme_id, nerd_font, area.width.saturating_sub(3)))
        .title_style(Style::default().fg(spec.text_normal))
        .style(Style::default().bg(spec.bg_panel).fg(spec.text_normal))
        .border_style(Style::default().fg(spec.divider));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Span::styled("x", Style::default().fg(spec.text_normal))),
        Rect::new(area.x + area.width.saturating_sub(2), area.y, 1, 1),
    );
    inner
}

/// Render a vertical scrollbar track into `sb_area`.
fn render_scrollbar_track(
    f: &mut Frame,
    sb_area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_offset: usize,
    max_scroll: usize,
) {
    let track_h = sb_area.height as usize;
    if track_h == 0 || total_rows == 0 {
        return;
    }
    let thumb_h = ((visible_rows * track_h + total_rows.saturating_sub(1)) / total_rows)
        .max(1)
        .min(track_h);
    let scroll_space = track_h.saturating_sub(thumb_h);
    let thumb_y = if max_scroll == 0 {
        0
    } else {
        (scroll_offset * scroll_space + (max_scroll / 2)) / max_scroll
    };
    let mut sb_lines: Vec<Line> = Vec::with_capacity(track_h);
    for row in 0..track_h {
        let in_thumb = row >= thumb_y && row < thumb_y + thumb_h;
        let (ch, color) = if in_thumb {
            ("┃", Color::Rgb(120, 240, 220))
        } else {
            ("│", Color::Rgb(80, 200, 180))
        };
        sb_lines.push(Line::from(Span::styled(ch, Style::default().fg(color))));
    }
    f.render_widget(Paragraph::new(sb_lines), sb_area);
}

/// Prepend a single space to each line (for left-padding inside overlay panels).
fn indent_lines<'a>(lines: &[Line<'a>]) -> Vec<Line<'a>> {
    lines
        .iter()
        .map(|line| {
            let mut spans: Vec<Span> = Vec::with_capacity(line.spans.len() + 1);
            spans.push(Span::raw(" "));
            spans.extend(line.spans.iter().cloned());
            Line::from(spans)
        })
        .collect()
}

pub fn panel_tab_hit_test(relative_x: u16, active: u8, avail_width: u16) -> Option<u8> {
    let (lo, hi, more_left, more_right) =
        visible_tab_window(active as usize, avail_width as usize);

    let mut cursor = 0u16;

    // Clicking the left chevron scrolls by selecting the tab just before the window.
    if more_left {
        if relative_x == cursor {
            return Some((lo - 1) as u8);
        }
        cursor = cursor.saturating_add(1);
    }

    for i in lo..=hi {
        if i > lo {
            if relative_x == cursor {
                return None;
            }
            cursor = cursor.saturating_add(1);
        }

        let width = tab_label_width(i) as u16;
        if relative_x >= cursor && relative_x < cursor.saturating_add(width) {
            return Some(PANEL_TABS[i].1);
        }
        cursor = cursor.saturating_add(width);
    }

    // Clicking the right chevron scrolls by selecting the tab just after the window.
    if more_right && relative_x == cursor {
        return Some((hi + 1) as u8);
    }

    None
}

/// Powerline rounded caps used to render shortcut keys as pills.
const PILL_LEFT_CAP: &str = "\u{e0b6}";
const PILL_RIGHT_CAP: &str = "\u{e0b4}";

/// Spans for a single footer shortcut: the key (a rounded pill when nerd fonts
/// are active) followed by a space and its description. No `:` separator.
pub fn shortcut_spans(
    key: &str,
    description: &str,
    nerd_font: bool,
    spec: &crate::ui::theme::ThemeSpec,
) -> Vec<Span<'static>> {
    let desc_style = Style::default().fg(spec.text_dim);
    let mut spans: Vec<Span<'static>> = Vec::new();

    if nerd_font {
        let pill_bg = spec.bg_selected;
        let cap_style = Style::default().fg(pill_bg);
        let key_style = Style::default()
            .fg(spec.text_normal)
            .bg(pill_bg)
            .add_modifier(Modifier::BOLD);
        spans.push(Span::styled(PILL_LEFT_CAP, cap_style));
        spans.push(Span::styled(key.to_string(), key_style));
        spans.push(Span::styled(PILL_RIGHT_CAP, cap_style));
    } else {
        let key_style = Style::default().fg(spec.text_normal).add_modifier(Modifier::BOLD);
        spans.push(Span::styled(key.to_string(), key_style));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(description.to_string(), desc_style));
    spans
}

/// Rendered display width of a shortcut produced by [`shortcut_spans`].
pub fn shortcut_width(key: &str, description: &str, nerd_font: bool) -> usize {
    let base = key.width() + 1 + description.width();
    if nerd_font {
        base + 2
    } else {
        base
    }
}

pub fn shortcut_footer_line(
    entries: &[(&'static str, &'static str)],
    theme_id: ThemeId,
    nerd_font: bool,
) -> Line<'static> {
    let spec = theme_spec(theme_id);
    let sep_style = Style::default().fg(spec.divider);
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];

    for (idx, (shortcut, description)) in entries.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled("  ", sep_style));
        }
        spans.extend(shortcut_spans(shortcut, description, nerd_font, spec));
    }

    Line::from(spans)
}

pub fn shortcut_footer_lines(
    entries: &[(&'static str, &'static str)],
    theme_id: ThemeId,
    nerd_font: bool,
) -> Vec<Line<'static>> {
    vec![Line::from(""), shortcut_footer_line(entries, theme_id, nerd_font)]
}

fn selector_edge_spans(is_selected: bool, spec: &crate::ui::theme::ThemeSpec) -> (Span<'static>, Span<'static>) {
    if is_selected {
        (
            Span::styled(
                "",
                Style::default().fg(spec.bg_selected).bg(spec.bg_panel),
            ),
            Span::styled(
                "",
                Style::default().fg(spec.bg_selected).bg(spec.bg_panel),
            ),
        )
    } else {
        (
            Span::styled(" ", Style::default().bg(spec.bg_panel)),
            Span::styled(" ", Style::default().bg(spec.bg_panel)),
        )
    }
}

/// Shared chrome for the tab-anchored overlays (integrations, sort): where the
/// overlay is anchored, which tab opened it, and the active theme / font mode.
#[derive(Clone, Copy)]
pub struct OverlayChrome {
    pub anchor: Rect,
    pub panel_tab: u8,
    pub theme_id: ThemeId,
    pub nerd_font: bool,
}

pub fn render_integrations_overlay(
    f: &mut Frame,
    area: Rect,
    chrome: OverlayChrome,
    integrations: &[IntegrationRow],
    integration_selected: usize,
) {
    let OverlayChrome {
        anchor: tab_overlay_anchor,
        panel_tab,
        theme_id,
        nerd_font,
    } = chrome;
    let spec = theme_spec(theme_id);
    let int_w = (area.width * 5 / 6).max(70).min(tab_overlay_anchor.width);
    let int_content_w = int_w.saturating_sub(2) as usize;
    let int_row_inner_w = int_content_w.saturating_sub(2);

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, row) in integrations.iter().enumerate() {
        let is_selected = i == integration_selected;
        let is_enabled = matches!(
            row.state.as_str(),
            "[required]" | "[active]" | "[partial]" | "[on]"
        );
        let status_text = if row.required || (is_enabled && row.available) {
            " ✓ ".to_string()
        } else if is_enabled && row.partially_supported {
            " ✓ ".to_string()
        } else {
            " ✕ ".to_string()
        };
        let status_style = if row.required || (is_enabled && row.available) {
            Style::default().fg(Color::Rgb(100, 220, 120))
        } else if is_enabled && row.partially_supported {
            Style::default().fg(Color::Rgb(245, 200, 90))
        } else {
            Style::default().fg(Color::Rgb(220, 80, 80))
        };
        let base_style = if is_selected {
            Style::default().bg(spec.bg_selected).fg(spec.text_normal)
        } else {
            Style::default().fg(spec.text_normal)
        };
        let name_text = format!("  {:<12}", row.label);
        let state_text = format!(" {:<10}", row.state);
        let category_text = format!(" {:<9}", row.category);
        let purpose_text = format!(" {}", row.description);

        let name_span = Span::styled(name_text.clone(), base_style);
        let state_span = Span::styled(
            state_text.clone(),
            if row.required {
                base_style.fg(Color::Rgb(200, 200, 200))
            } else if !row.available && !row.partially_supported {
                base_style.fg(Color::Rgb(220, 80, 80))
            } else if is_enabled && row.partially_supported {
                base_style.fg(Color::Rgb(245, 200, 90))
            } else if is_enabled {
                base_style.fg(Color::Rgb(255, 220, 140))
            } else {
                base_style.fg(spec.text_dim)
            },
        );
        let category_span = Span::styled(category_text.clone(), base_style);
        let purpose_span = Span::styled(purpose_text.clone(), base_style);
        let (left_cap, right_cap) = selector_edge_spans(is_selected, spec);
        let mut spans = vec![
            left_cap,
            Span::styled(status_text.clone(), base_style.patch(status_style)),
            name_span,
            state_span,
            category_span,
            purpose_span,
        ];

        if is_selected {
            let used_w = UnicodeWidthStr::width(status_text.as_str())
                + UnicodeWidthStr::width(name_text.as_str())
                + UnicodeWidthStr::width(state_text.as_str())
                + UnicodeWidthStr::width(category_text.as_str())
                + UnicodeWidthStr::width(purpose_text.as_str());
            if int_row_inner_w > used_w {
                spans.push(Span::styled(" ".repeat(int_row_inner_w - used_w), base_style));
            }
        }

        spans.push(right_cap);

        lines.push(Line::from(spans));
    }
    let int_h = (lines.len() as u16 + 4).min(tab_overlay_anchor.height);
    let int_area = Rect::new(tab_overlay_anchor.x, tab_overlay_anchor.y, int_w, int_h);
    f.render_widget(Clear, int_area);
    let int_inner = render_overlay_block(f, int_area, panel_tab, theme_id, nerd_font);
    let int_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(int_inner);
    let visible_rows = int_chunks[0].height as usize;
    let total_rows = lines.len();
    let max_scroll = total_rows.saturating_sub(visible_rows);
    let selected_line = integration_selected + 1;
    let int_scroll = (selected_line + 1).saturating_sub(visible_rows)
    .min(max_scroll);
    let can_draw_scrollbar = int_chunks[0].width > 2 && total_rows > visible_rows;

    f.render_widget(
        Paragraph::new(indent_lines(&lines)).scroll((int_scroll as u16, 0)),
        int_chunks[0],
    );
    if can_draw_scrollbar {
        let sb_area = Rect::new(
            int_area.x + int_area.width.saturating_sub(1),
            int_chunks[0].y,
            1,
            int_chunks[0].height,
        );
        render_scrollbar_track(f, sb_area, total_rows, visible_rows, int_scroll, max_scroll);
    }
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(&[
            ("↑↓", "navigate"),
            ("Space", "toggle"),
            ("Enter", "install missing"),
            ("Tab", "switch tabs"),
            ("Esc", "close"),
        ], theme_id, nerd_font)),
        int_chunks[1],
    );
}

pub fn render_help_overlay(
    f: &mut Frame,
    tab_overlay_anchor: Rect,
    panel_tab: u8,
    theme_id: ThemeId,
    help_scroll_offset: u16,
    nerd_font: bool,
) -> (u16, u16) {
    let help_w = tab_overlay_anchor.width;
    let inner_w = help_w.saturating_sub(4) as usize;
    let shortcut_w = inner_w.clamp(10, 18);
    let section_style = Style::default().fg(Color::Rgb(120, 200, 255)).add_modifier(Modifier::BOLD);
    let shortcut_style = Style::default().fg(Color::Rgb(255, 220, 140)).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::Rgb(200, 200, 200));

    let config_path = {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })
            .unwrap_or_else(|| PathBuf::from(".config"));
        base.join("sb").join("config")
    };

    let title_style = Style::default().fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD);
    let subtitle_style = Style::default().fg(theme_spec(theme_id).text_dim);

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            format!("Shell Buddy  v{}", env!("CARGO_PKG_VERSION")),
            title_style,
        )),
        Line::from(Span::styled(
            config_path.display().to_string(),
            subtitle_style,
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{:<width$}", "Shortcut", width = shortcut_w),
                Style::default().fg(Color::Rgb(190, 190, 190)).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Description", Style::default().fg(Color::Rgb(190, 190, 190)).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let sections: [(&str, [(&str, &str); 10]); 5] = [
        (
            "Navigation & View",
            [
                ("Up / Down", "Move selection"),
                ("PgUp / PgDn", "Jump by visible page"),
                ("Home / End", "Jump to first or last item"),
                ("Enter / Right", "Open folder/file or preview"),
                ("Left / Bksps", "Go to parent folder"),
                ("Mouse Click", "L: Select | Double L: Open | R: Parent folder"),
                ("Tab / ~", "Edit path (or switch pane in preview) | Go home"),
                ("` / s", "Toggle preview | Toggle folder size calc"),
                ("Ctrl+s", "Open sorting menu"),
                (".", "Toggle hidden files"),
            ],
        ),
        (
            "Selection & Metadata",
            [
                ("Space / Ins", "Toggle mark for selected item"),
                ("*", "Toggle all marks in directory"),
                ("Ctrl+n", "Add/edit note for selected item"),
                ("Ctrl+c", "Copy full path(s) to system clipboard"),
                ("Ctrl+e", "Edit system clipboard via temp file"),
                ("", ""),
                ("", ""),
                ("", ""),
                ("", ""),
                ("", ""),
            ],
        ),
        (
            "File Operations",
            [
                ("n", "New 'file' or '/folder'"),
                ("c / F5", "Copy marked to app clipboard"),
                ("v / m", "Paste / Move clipboard to folder"),
                ("r / F2", "Rename or bulk rename"),
                ("e / F4", "Edit file (or rename folder)"),
                ("d / Del", "Delete selected item(s)"),
                ("x / ", "Toggle executable flag"),
                ("Z", "Create or extract archive"),
                ("o", "Open with default GUI app"),
                ("p", "protect file with age"),
            ],
        ),
        (
            "Search & External",
            [
                ("f / g", "Fuzzy search | Content search"),
                ("w", "Download URL (prompt: Ctrl+V or right-click pastes from system clipboard)"),
                ("S", "Open SSH/rclone mount picker"),
                ("C", "Delta compare (marked vs cursor)"),
                ("i / E", "Split shell (L) + preview/edit (R)"),
                ("I", "Open integrations panel"),
                ("b / 0-9", "Open bookmarks | Jump to bookmark"),
                ("", ""),
                ("", ""),
                ("", ""),
            ],
        ),
        (
            "System & Git",
            [
                ("G", "Git: Commit + Push (dirty repos)"),
                ("H", "Git: View pretty log graph"),
                ("Ctrl+z", "Drop to shell in current directory"),
                ("t", "Open ~/.todo in $EDITOR"),
                ("h / ?", "Open this help screen"),
                ("q / Esc", "Quit Shell Buddy"),
                ("", ""),
                ("", ""),
                ("", ""),
                ("", ""),
            ],
        ),
    ];
    for (section_title, rows) in sections {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(section_title.to_string(), section_style)));
        for (shortcut, description) in rows {
            if shortcut.is_empty() && description.is_empty() {
                continue;
            }
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<width$}", shortcut, width = shortcut_w),
                    shortcut_style,
                ),
                Span::styled(description.to_string(), desc_style),
            ]));
        }
    }

    let desired_h = (lines.len() as u16 + 4).max(18);
    let help_h = desired_h.min(tab_overlay_anchor.height);
    let help_area = Rect::new(
        tab_overlay_anchor.x,
        tab_overlay_anchor.y,
        help_w,
        help_h,
    );
    f.render_widget(Clear, help_area);

    let help_inner = render_overlay_block(f, help_area, panel_tab, theme_id, nerd_font);
    let help_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(help_inner);
    let help_content_area = help_chunks[0];
    let help_footer_area = help_chunks[1];

    let needs_scroll = lines.len() > help_content_area.height as usize;
    let can_draw_scrollbar = help_content_area.width > 2 && needs_scroll;
    let help_text_area = help_content_area;

    let visible_lines = help_text_area.height as usize;
    let total_lines = lines.len();
    let max_scroll = total_lines.saturating_sub(visible_lines);
    let max_offset = max_scroll as u16;
    let clamped_offset = (help_scroll_offset as usize).min(max_scroll) as u16;
    f.render_widget(
        Paragraph::new(indent_lines(&lines))
            .wrap(Wrap { trim: false })
            .scroll((clamped_offset, 0)),
        help_text_area,
    );
    if can_draw_scrollbar {
        let sb_area = Rect::new(
            help_area.x + help_area.width.saturating_sub(1),
            help_content_area.y,
            1,
            help_content_area.height,
        );
        render_scrollbar_track(
            f, sb_area, total_lines, visible_lines,
            clamped_offset as usize, max_scroll,
        );
    }
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(&[
            ("↑↓", "navigate"),
            ("Tab", "switch tabs"),
            ("c", "open config"),
            ("Esc", "close"),
        ], theme_id, nerd_font)),
        help_footer_area,
    );

    (max_offset, clamped_offset)
}

pub fn render_bookmarks_overlay(
    f: &mut Frame,
    tab_overlay_anchor: Rect,
    panel_tab: u8,
    theme_id: ThemeId,
    bookmarks: &[(usize, Option<PathBuf>)],
    bookmark_selected: usize,
    nerd_font: bool,
) {
    let spec = theme_spec(theme_id);
    let mut lines: Vec<Line> = vec![Line::from("")];
    let bm_w = tab_overlay_anchor.width;
    let bm_content_w = bm_w.saturating_sub(2) as usize;
    let bm_row_inner_w = bm_content_w.saturating_sub(2);
    for (row_idx, (i, path)) in bookmarks.iter().enumerate() {
        let is_selected = row_idx == bookmark_selected;
        let base_style = if is_selected {
            Style::default().bg(spec.bg_selected).fg(spec.text_normal)
        } else {
            Style::default()
        };

        let (label, style) = match path {
            Some(p) => (
                format!(" [{}]  {}", i, p.display()),
                Style::default().fg(Color::Rgb(100, 220, 120)).patch(base_style),
            ),
            None => (
                format!(" [{}]  (not set)", i),
                Style::default().fg(Color::Rgb(80, 80, 80)).patch(base_style),
            ),
        };

        let padded_label = if is_selected {
            let used_w = UnicodeWidthStr::width(label.as_str());
            if bm_row_inner_w > used_w {
                format!("{}{}", label, " ".repeat(bm_row_inner_w - used_w))
            } else {
                label
            }
        } else {
            label
        };
        let (left_cap, right_cap) = selector_edge_spans(is_selected, spec);
        lines.push(Line::from(vec![
            left_cap,
            Span::styled(padded_label, style),
            right_cap,
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" Add to your shell config to set bookmarks:", Style::default().fg(Color::Rgb(200, 180, 80)))));
    lines.push(Line::from(Span::styled("  export SB_BOOKMARK_1=\"$HOME/.config\"", Style::default().fg(spec.text_dim))));
    lines.push(Line::from(Span::styled("  export SB_BOOKMARK_2=\"/var/log\"", Style::default().fg(spec.text_dim))));
    let bm_h = (lines.len() as u16 + 4).max(17).min(tab_overlay_anchor.height);
    let bm_area = Rect::new(
        tab_overlay_anchor.x,
        tab_overlay_anchor.y,
        bm_w,
        bm_h,
    );
    f.render_widget(Clear, bm_area);
    let bm_inner = render_overlay_block(f, bm_area, panel_tab, theme_id, nerd_font);
    let bm_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(bm_inner);
    f.render_widget(Paragraph::new(lines), bm_chunks[0]);
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(&[
            ("↑↓", "navigate"),
            ("Enter/0-9", "jump"),
            ("Tab", "switch tabs"),
            ("Esc", "close"),
        ], theme_id, nerd_font)),
        bm_chunks[1],
    );
}

pub fn render_sort_overlay(
    f: &mut Frame,
    chrome: OverlayChrome,
    options: &[SortMode],
    sort_menu_selected: usize,
    current_sort_mode: SortMode,
) {
    let OverlayChrome {
        anchor: tab_overlay_anchor,
        panel_tab,
        theme_id,
        nerd_font: nerd_font_active,
    } = chrome;
    let spec = theme_spec(theme_id);
    let sort_w = tab_overlay_anchor.width;
    let sort_content_w = sort_w.saturating_sub(2) as usize;
    let sort_row_inner_w = sort_content_w.saturating_sub(2);
    let mut lines: Vec<Line> = vec![Line::from("")];
    for (idx, mode) in options.iter().enumerate() {
        let is_selected = idx == sort_menu_selected;
        let is_current = *mode == current_sort_mode;
        let (nerd_icon, fallback_icon) = match mode {
            SortMode::NameAsc => ("\u{f15d}", "[A-Z]"),
            SortMode::NameDesc => ("\u{f15e}", "[Z-A]"),
            SortMode::ExtensionAsc => ("\u{f1c9}", "[EXT]"),
            SortMode::SizeAsc => ("\u{f160}", "[SZ+]"),
            SortMode::SizeDesc => ("\u{f161}", "[SZ-]"),
            SortMode::ModifiedNewest => ("\u{f017}", "[NEW]"),
            SortMode::ModifiedOldest => ("\u{f1da}", "[OLD]"),
        };
        let sort_icon = if nerd_font_active {
            nerd_icon
        } else {
            fallback_icon
        };
        let row_text = format!(" {}  {}", sort_icon, mode.label());
        let row_text = if is_selected {
            let used_w = UnicodeWidthStr::width(row_text.as_str());
            if sort_row_inner_w > used_w {
                format!("{}{}", row_text, " ".repeat(sort_row_inner_w - used_w))
            } else {
                row_text
            }
        } else {
            row_text
        };
        let style = if is_selected {
            Style::default().bg(spec.bg_selected).fg(spec.text_normal)
        } else if is_current {
            Style::default().fg(spec.warning)
        } else {
            Style::default().fg(spec.text_normal)
        };
        let (left_cap, right_cap) = selector_edge_spans(is_selected, spec);
        lines.push(Line::from(vec![
            left_cap,
            Span::styled(row_text, style),
            right_cap,
        ]));
    }

    let sort_h = (lines.len() as u16 + 4).max(10).min(tab_overlay_anchor.height);
    let sort_area = Rect::new(
        tab_overlay_anchor.x,
        tab_overlay_anchor.y,
        sort_w,
        sort_h,
    );
    f.render_widget(Clear, sort_area);
    let sort_inner = render_overlay_block(f, sort_area, panel_tab, theme_id, nerd_font_active);
    let sort_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(sort_inner);
    f.render_widget(Paragraph::new(lines), sort_chunks[0]);
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(&[
            ("↑↓", "navigate"),
            ("Enter", "apply"),
            ("Tab", "switch tabs"),
            ("Esc", "close"),
        ], theme_id, nerd_font_active)),
        sort_chunks[1],
    );
}

pub fn render_themes_overlay(
    f: &mut Frame,
    tab_overlay_anchor: Rect,
    panel_tab: u8,
    theme_id: ThemeId,
    selected: usize,
    nerd_font: bool,
) {
    let current = theme_spec(theme_id);
    let theme_w = tab_overlay_anchor.width;
    let theme_content_w = theme_w.saturating_sub(2) as usize;
    let theme_row_inner_w = theme_content_w.saturating_sub(2);
    // Width of the name column = longest theme name (display width), so the
    // checkboxes line up regardless of how long custom skin names are.
    let name_col_w = themes()
        .iter()
        .map(|t| UnicodeWidthStr::width(t.name))
        .max()
        .unwrap_or(0)
        .max(8);
    let mut lines: Vec<Line> = vec![Line::from("")];
    for (idx, theme) in themes().iter().enumerate() {
        let is_selected = idx == selected;
        let is_applied = theme.id == theme_id;
        let spec = theme_spec(theme.id);
        let base_style = if is_selected {
            Style::default().bg(current.bg_selected).fg(current.text_normal)
        } else {
            Style::default().fg(current.text_normal)
        };
        let name_pad = name_col_w.saturating_sub(UnicodeWidthStr::width(theme.name));
        let row_text = format!(
            " {}{} {}",
            theme.name,
            " ".repeat(name_pad),
            if is_applied { "[x]" } else { "[ ]" }
        );
        let row_text_w = UnicodeWidthStr::width(row_text.as_str());
        let swatch_bg = if is_selected {
            Style::default().bg(current.bg_selected)
        } else {
            Style::default()
        };
        let (left_cap, right_cap) = selector_edge_spans(is_selected, current);
        let mut row = vec![
            left_cap,
            Span::styled(row_text, base_style),
            Span::styled("  ", swatch_bg),
            Span::styled("bg", Style::default().bg(spec.bg_panel).fg(spec.text_normal)),
            Span::styled(" ", swatch_bg),
            Span::styled("██", swatch_bg.fg(spec.text_normal)),
            Span::styled(" ", swatch_bg),
            Span::styled("██", swatch_bg.fg(spec.accent_primary)),
            Span::styled(" ", swatch_bg),
            Span::styled("██", swatch_bg.fg(spec.success)),
            Span::styled(" ", swatch_bg),
            Span::styled("██", swatch_bg.fg(spec.warning)),
            Span::styled(" ", swatch_bg),
            Span::styled("██", swatch_bg.fg(spec.error)),
        ];
        if is_selected {
            let used_w = row_text_w + 19;
            if theme_row_inner_w > used_w {
                row.push(Span::styled(" ".repeat(theme_row_inner_w - used_w), swatch_bg));
            }
        }
        row.push(right_cap);
        lines.push(Line::from(row));
    }

    let theme_h = (lines.len() as u16 + 7).max(12).min(tab_overlay_anchor.height);
    let theme_area = Rect::new(tab_overlay_anchor.x, tab_overlay_anchor.y, theme_w, theme_h);
    f.render_widget(Clear, theme_area);
    let theme_inner = render_overlay_block(f, theme_area, panel_tab, theme_id, nerd_font);
    let theme_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(theme_inner);
    f.render_widget(Paragraph::new(lines), theme_chunks[0]);
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(&[
            ("↑↓", "select"),
            ("Enter/Space", "apply"),
            ("T", "open themes"),
            ("Esc", "close"),
        ], theme_id, nerd_font)),
        theme_chunks[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // Full padded width of all 7 tabs plus 6 single-column separators.
    fn full_width() -> usize {
        (0..PANEL_TABS.len()).map(tab_label_width).sum::<usize>() + PANEL_TABS.len() - 1
    }

    #[test]
    fn window_shows_all_tabs_when_space_is_ample() {
        let avail = full_width();
        assert_eq!(visible_tab_window(3, avail), (0, 6, false, false));
        assert_eq!(visible_tab_window(0, avail + 50), (0, 6, false, false));
    }

    #[test]
    fn window_keeps_active_visible_and_flags_hidden_edges() {
        // Narrow bar: active at the far right must stay in view.
        let (lo, hi, more_left, more_right) = visible_tab_window(6, 30);
        assert!(lo <= 6 && hi == 6, "active 6 must be visible: {lo}..={hi}");
        assert!(more_left, "tabs are hidden to the left");
        assert!(!more_right, "nothing hidden past the last tab");

        // Active at the far left.
        let (lo, hi, more_left, more_right) = visible_tab_window(0, 30);
        assert_eq!(lo, 0);
        assert!(hi < 6);
        assert!(!more_left);
        assert!(more_right);
    }

    #[test]
    fn hit_test_maps_visible_tabs_and_chevrons() {
        // Wide: behaves like a plain, fully-rendered bar.
        let avail = full_width() as u16;
        assert_eq!(panel_tab_hit_test(0, 0, avail), Some(0)); // " Help "
        assert_eq!(panel_tab_hit_test(6, 0, avail), None); // separator
        assert_eq!(panel_tab_hit_test(7, 0, avail), Some(1)); // " Search "

        // Narrow + active=6 → window is tabs 5..=6 with a left chevron at x0.
        let (lo, hi, more_left, more_right) = visible_tab_window(6, 30);
        assert_eq!((lo, hi, more_left, more_right), (5, 6, true, false));
        assert_eq!(panel_tab_hit_test(0, 6, 30), Some(4)); // left chevron → tab before window
        assert_eq!(panel_tab_hit_test(1, 6, 30), Some(5)); // first visible tab
        assert_eq!(panel_tab_hit_test(16, 6, 30), Some(6)); // active tab
    }
}
