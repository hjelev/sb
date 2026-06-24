use crate::integration::rows::IntegrationRow;
use crate::ui::theme::{theme_spec, themes, ThemeId};
use crate::SortMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use std::path::PathBuf;
use std::sync::OnceLock;
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
    spec: &crate::ui::theme::ThemeSpec,
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
            ("┃", spec.accent_primary)
        } else {
            ("│", spec.divider)
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

/// Darken a color toward black for the label pill background.
/// Named colors (e.g. the Original theme's `DarkGray`) fall back to a fixed
/// near-black so the label pill always reads darker than the key pill.
fn darken(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * factor) as u8,
            (g as f32 * factor) as u8,
            (b as f32 * factor) as u8,
        ),
        _ => Color::Rgb(24, 24, 30),
    }
}

/// Spans for a single footer shortcut: a single two-tone pill. The key segment
/// (lighter `bg_selected`) flows straight into the darker label segment, sharing
/// one rounded outline — rounded caps only on the outer left/right ends. With
/// nerd fonts the ends are Powerline caps; without, it degrades to a square
/// two-tone background block. No `:` separator.
pub fn shortcut_spans(
    key: &str,
    description: &str,
    nerd_font: bool,
    spec: &crate::ui::theme::ThemeSpec,
) -> Vec<Span<'static>> {
    let key_bg = spec.bg_selected;
    let label_bg = darken(key_bg, 0.45);
    let key_style = Style::default()
        .fg(spec.text_normal)
        .bg(key_bg)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(spec.text_normal).bg(label_bg);
    let mut spans: Vec<Span<'static>> = Vec::new();

    if nerd_font {
        // Outer left cap: gray rounding out of the footer background.
        spans.push(Span::styled(PILL_LEFT_CAP, Style::default().fg(key_bg)));
        spans.push(Span::styled(key.to_string(), key_style));
        // Junction cap: gray rounded edge drawn over the dark label background,
        // so the key segment appears to round into the darker area.
        spans.push(Span::styled(
            PILL_RIGHT_CAP,
            Style::default().fg(key_bg).bg(label_bg),
        ));
        spans.push(Span::styled(description.to_string(), label_style));
        // Outer right cap: dark rounding back into the footer background.
        spans.push(Span::styled(PILL_RIGHT_CAP, Style::default().fg(label_bg)));
    } else {
        spans.push(Span::styled(key.to_string(), key_style));
        spans.push(Span::styled(description.to_string(), label_style));
    }
    spans
}

/// Rendered display width of a shortcut produced by [`shortcut_spans`].
///
/// Compact: no inner padding. Nerd fonts add three rounded caps (left,
/// junction, right) around the key and label text; the square fallback is just
/// the two text segments.
pub fn shortcut_width(key: &str, description: &str, nerd_font: bool) -> usize {
    if nerd_font {
        key.width() + description.width() + 3
    } else {
        key.width() + description.width()
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

/// Map a footer key label (as shown in the pill) to the key event a click
/// should synthesize. Pure navigation labels like the arrow pair are not
/// actionable and return `None`; combined labels (e.g. "Enter/0-9",
/// "Enter/Space") map to their primary key.
fn footer_key_label_to_event(label: &str) -> Option<KeyEvent> {
    // Modifier-prefixed labels like "Ctrl+T". Crossterm delivers Ctrl+<letter>
    // as the lowercase char with the CONTROL modifier, so normalize to that.
    if let Some(rest) = label.strip_prefix("Ctrl+") {
        let mut chars = rest.chars();
        let c = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        return Some(KeyEvent::new(
            KeyCode::Char(c.to_ascii_lowercase()),
            KeyModifiers::CONTROL,
        ));
    }
    // Combined "X/Y" labels (e.g. "Enter/0-9", "Enter/→", "u/Delete") use the
    // first segment as the primary key. The lone "/" search key has no leading
    // segment, so fall back to the whole label in that case.
    let primary = match label.split('/').next() {
        Some(seg) if !seg.is_empty() => seg,
        _ => label,
    };
    let code = if primary == "Space" {
        KeyCode::Char(' ')
    } else if primary == "Tab" {
        KeyCode::Tab
    } else if primary == "Esc" {
        KeyCode::Esc
    } else if primary == "Enter" {
        KeyCode::Enter
    } else {
        let mut chars = primary.chars();
        let c = chars.next()?;
        if chars.next().is_some() {
            return None; // multi-char, non-keyword label (e.g. "↑↓", "Regex")
        }
        KeyCode::Char(c)
    };
    Some(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Compute clickable hit-zones for a shortcut footer produced by
/// [`shortcut_footer_lines`] when rendered into `footer_area`. The layout
/// mirrors [`shortcut_footer_line`]: a blank first row, then a leading space
/// followed by pills separated by two spaces. Each returned tuple is
/// `(event, x_start, x_end_exclusive, y)` in terminal cells.
pub fn footer_shortcut_zones(
    entries: &[(&'static str, &'static str)],
    footer_area: Rect,
    nerd_font: bool,
) -> Vec<(KeyEvent, u16, u16, u16)> {
    let y = footer_area.y + 1; // blank Line::from("") precedes the shortcut line
    let mut x = footer_area.x + 1; // leading Span::raw(" ")
    let mut zones = Vec::new();
    for (idx, (key, desc)) in entries.iter().enumerate() {
        if idx > 0 {
            x = x.saturating_add(2); // two-space separator between pills
        }
        let w = shortcut_width(key, desc, nerd_font) as u16;
        if let Some(event) = footer_key_label_to_event(key) {
            zones.push((event, x, x.saturating_add(w), y));
        }
        x = x.saturating_add(w);
    }
    zones
}

fn selector_edge_spans(is_selected: bool, spec: &crate::ui::theme::ThemeSpec, nerd_font: bool) -> (Span<'static>, Span<'static>) {
    if is_selected {
        if nerd_font {
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
                Span::styled(" ", Style::default().bg(spec.bg_selected)),
                Span::styled(" ", Style::default().bg(spec.bg_selected)),
            )
        }
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
    search_active: bool,
    search_query: &str,
    show_icons: bool,
    footer_zones: &mut Vec<(KeyEvent, u16, u16, u16)>,
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

    // The first line is either a blank spacer or the live search bar. Keeping it as
    // a single leading line preserves the row indexing used by mouse click handling.
    let first_line = if search_active {
        let query_icon = if show_icons && nerd_font { "\u{f002}" } else { "/" };
        Line::from(vec![
            Span::styled(format!("  {}  ", query_icon), Style::default().fg(spec.key_label)),
            Span::styled(search_query.to_string(), Style::default().fg(spec.text_normal)),
            Span::styled("▏", Style::default().fg(spec.key_label)),
        ])
    } else {
        Line::from("")
    };
    let mut lines: Vec<Line> = vec![first_line];
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
            Style::default().fg(spec.success)
        } else if is_enabled && row.partially_supported {
            Style::default().fg(spec.warning)
        } else {
            Style::default().fg(spec.error)
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
                base_style.fg(spec.text_normal)
            } else if !row.available && !row.partially_supported {
                base_style.fg(spec.error)
            } else if is_enabled && row.partially_supported {
                base_style.fg(spec.warning)
            } else if is_enabled {
                base_style.fg(spec.key_label)
            } else {
                base_style.fg(spec.text_dim)
            },
        );
        let category_span = Span::styled(category_text.clone(), base_style);
        let purpose_span = Span::styled(purpose_text.clone(), base_style);
        let (left_cap, right_cap) = selector_edge_spans(is_selected, spec, nerd_font);
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
        render_scrollbar_track(f, sb_area, total_rows, visible_rows, int_scroll, max_scroll, spec);
    }
    let footer_entries: &[(&'static str, &'static str)] = &[
        ("↑↓", "navigate"),
        ("Space", "toggle"),
        ("Enter", "install missing"),
        ("/", "search"),
        ("Tab", "switch tabs"),
        ("Esc", "close"),
    ];
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(footer_entries, theme_id, nerd_font)),
        int_chunks[1],
    );
    footer_zones.extend(footer_shortcut_zones(footer_entries, int_chunks[1], nerd_font));
}

const HELP_LOGO_BYTES: &[u8] = include_bytes!("../../docs/images/favicon.png");
static HELP_LOGO_RGBA: OnceLock<Option<(Vec<u8>, u32, u32)>> = OnceLock::new();

fn help_logo_rgba() -> Option<&'static (Vec<u8>, u32, u32)> {
    HELP_LOGO_RGBA
        .get_or_init(|| {
            image::load_from_memory(HELP_LOGO_BYTES).ok().map(|img| {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width(), rgba.height());
                (rgba.into_raw(), w, h)
            })
        })
        .as_ref()
}

/// Alpha-blend the embedded logo's RGBA pixels against `bg` (the PNG has
/// transparent corners, so this avoids a black/white box around the glyph
/// when composited at low resolution or via Sixel, which has no alpha channel).
fn help_logo_rgb_blend(bg: Color) -> Option<(Vec<u8>, u32, u32)> {
    let (rgba, w, h) = help_logo_rgba()?;
    let (bg_r, bg_g, bg_b) = match bg {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    };
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.chunks_exact(4) {
        let af = px[3] as f32 / 255.0;
        rgb.push((px[0] as f32 * af + bg_r as f32 * (1.0 - af)).round() as u8);
        rgb.push((px[1] as f32 * af + bg_g as f32 * (1.0 - af)).round() as u8);
        rgb.push((px[2] as f32 * af + bg_b as f32 * (1.0 - af)).round() as u8);
    }
    Some((rgb, *w, *h))
}

/// Render the embedded logo as `cols`×`rows` halfblock terminal cells (fallback
/// path for terminals without a native image protocol).
fn help_logo_lines(cols: u16, rows: u16, bg: Color) -> Vec<Line<'static>> {
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    let Some((rgb, w, h)) = help_logo_rgb_blend(bg) else {
        return Vec::new();
    };
    crate::App::halfblock_lines(&rgb, w, h, cols, rows)
}

/// Raw PNG bytes + native pixel dimensions of the embedded logo, for Kitty/iTerm2
/// transmission (these protocols composite PNG alpha themselves, so no pre-blend
/// is needed — unlike the halfblock/Sixel paths).
pub(crate) fn help_logo_png_bytes_and_dims() -> Option<(&'static [u8], u32, u32)> {
    let (_, w, h) = help_logo_rgba()?;
    Some((HELP_LOGO_BYTES, *w, *h))
}

/// Full-resolution RGB pixels of the embedded logo, alpha-blended against `bg`,
/// for Sixel transmission (Sixel has no alpha channel).
pub(crate) fn help_logo_rgb_for_sixel(bg: Color) -> Option<(Vec<u8>, u32, u32)> {
    help_logo_rgb_blend(bg)
}

pub fn render_help_overlay(
    f: &mut Frame,
    tab_overlay_anchor: Rect,
    panel_tab: u8,
    theme_id: ThemeId,
    help_scroll_offset: u16,
    nerd_font: bool,
    footer_zones: &mut Vec<(KeyEvent, u16, u16, u16)>,
) -> (u16, u16, Option<Rect>) {
    let spec = theme_spec(theme_id);
    let help_w = tab_overlay_anchor.width;
    let inner_w = help_w.saturating_sub(4) as usize;
    let shortcut_w = inner_w.clamp(10, 18);
    let section_style = Style::default().fg(spec.overlay_section).add_modifier(Modifier::BOLD);
    let shortcut_style = Style::default().fg(spec.key_label).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(spec.text_normal);

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

    let title_style = Style::default().fg(spec.text_normal).add_modifier(Modifier::BOLD);
    let subtitle_style = Style::default().fg(spec.text_dim);

    let title_text = format!("Shell Buddy  v{}", env!("CARGO_PKG_VERSION"));
    let subtitle_text = config_path.display().to_string();
    let title_line = Line::from(Span::styled(title_text.clone(), title_style));
    let subtitle_line = Line::from(Span::styled(subtitle_text.clone(), subtitle_style));

    // The title/subtitle rows are always `lines[LOGO_TITLE_IDX]` /
    // `lines[LOGO_SUBTITLE_IDX]` below — used both to embed the logo and to
    // compute where it lands on screen for native-protocol overlay placement.
    const LOGO_TITLE_IDX: usize = 1;
    const LOGO_SUBTITLE_IDX: usize = 2;
    // favicon.png is square (1:1); at 2 text rows (4 pixel rows in halfblock
    // terms) that fits in 4 columns without letterboxing.
    const LOGO_COLS: u16 = 4;
    const LOGO_ROWS: u16 = 2;

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        title_line,
        subtitle_line,
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{:<width$}", "Shortcut", width = shortcut_w),
                Style::default().fg(spec.text_dim).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Description", Style::default().fg(spec.text_dim).add_modifier(Modifier::BOLD)),
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
                ("/", "Filter folder by name/regex (↓ list, ↑ box, Esc clear)"),
                ("w", "Download URL (prompt: Ctrl+V or right-click pastes from system clipboard)"),
                ("S", "Open SSH/rclone mount picker"),
                ("C", "Delta compare (marked vs cursor)"),
                ("i / E", "Split shell (L) + preview/edit (R)"),
                ("I", "Open integrations panel"),
                ("b / 0-9", "Open bookmarks | Jump to bookmark"),
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

    // The title/subtitle rows only sit at a known, stable screen position when
    // unscrolled (offset 0); once the list scrolls, those logical lines move
    // off-screen and `indent_lines`' single-row-per-`Line` assumption (no wrap)
    // must hold for the math below, so also require both rows fit unwrapped.
    let indent_w = 1usize;
    let needed_w = indent_w
        + LOGO_COLS as usize
        + 1
        + UnicodeWidthStr::width(title_text.as_str()).max(UnicodeWidthStr::width(subtitle_text.as_str()));
    let logo_rows_visible = clamped_offset == 0
        && help_text_area.height as usize > LOGO_SUBTITLE_IDX
        && needed_w <= help_text_area.width as usize;

    let native_protocol = crate::App::terminal_image_protocol().0;
    let native_supported = logo_rows_visible
        && matches!(
            native_protocol,
            crate::integration::probe::TerminalImageProtocol::Kitty
                | crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel
        )
        && help_logo_rgba().is_some();

    let logo_native_area = if native_supported {
        // Leave a blank hole the width of the logo; the real pixels are drawn
        // via the native protocol after ratatui's frame is flushed.
        let hole = Span::raw(" ".repeat(LOGO_COLS as usize));
        lines[LOGO_TITLE_IDX] = Line::from(vec![hole.clone(), Span::raw(" "), Span::styled(title_text.clone(), title_style)]);
        lines[LOGO_SUBTITLE_IDX] = Line::from(vec![hole, Span::raw(" "), Span::styled(subtitle_text.clone(), subtitle_style)]);
        Some(Rect::new(
            help_text_area.x + indent_w as u16,
            help_text_area.y + LOGO_TITLE_IDX as u16,
            LOGO_COLS,
            LOGO_ROWS,
        ))
    } else {
        if logo_rows_visible {
            let logo_lines = help_logo_lines(LOGO_COLS, LOGO_ROWS, spec.bg_panel);
            if logo_lines.len() == 2 {
                let mut top: Vec<Span> = logo_lines[0].spans.clone();
                top.push(Span::raw(" "));
                top.push(Span::styled(title_text.clone(), title_style));
                let mut bottom: Vec<Span> = logo_lines[1].spans.clone();
                bottom.push(Span::raw(" "));
                bottom.push(Span::styled(subtitle_text.clone(), subtitle_style));
                lines[LOGO_TITLE_IDX] = Line::from(top);
                lines[LOGO_SUBTITLE_IDX] = Line::from(bottom);
            }
        }
        None
    };

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
            clamped_offset as usize, max_scroll, spec,
        );
    }
    let footer_entries: &[(&'static str, &'static str)] = &[
        ("↑↓", "navigate"),
        ("Tab", "switch tabs"),
        ("c", "open config"),
        ("Esc", "close"),
    ];
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(footer_entries, theme_id, nerd_font)),
        help_footer_area,
    );
    footer_zones.extend(footer_shortcut_zones(footer_entries, help_footer_area, nerd_font));

    (max_offset, clamped_offset, logo_native_area)
}

pub fn render_bookmarks_overlay(
    f: &mut Frame,
    tab_overlay_anchor: Rect,
    panel_tab: u8,
    theme_id: ThemeId,
    bookmarks: &[(usize, Option<PathBuf>)],
    bookmark_selected: usize,
    nerd_font: bool,
    footer_zones: &mut Vec<(KeyEvent, u16, u16, u16)>,
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
                Style::default().fg(spec.success).patch(base_style),
            ),
            None => (
                format!(" [{}]  (not set)", i),
                Style::default().fg(spec.text_dim).patch(base_style),
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
        let (left_cap, right_cap) = selector_edge_spans(is_selected, spec, nerd_font);
        lines.push(Line::from(vec![
            left_cap,
            Span::styled(padded_label, style),
            right_cap,
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" Add to your shell config to set bookmarks:", Style::default().fg(spec.warning))));
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
    let footer_entries: &[(&'static str, &'static str)] = &[
        ("↑↓", "navigate"),
        ("Enter/0-9", "jump"),
        ("d", "delete"),
        ("Tab", "switch tabs"),
        ("Esc", "close"),
    ];
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(footer_entries, theme_id, nerd_font)),
        bm_chunks[1],
    );
    footer_zones.extend(footer_shortcut_zones(footer_entries, bm_chunks[1], nerd_font));
}

pub fn render_sort_overlay(
    f: &mut Frame,
    chrome: OverlayChrome,
    options: &[SortMode],
    sort_menu_selected: usize,
    current_sort_mode: SortMode,
    footer_zones: &mut Vec<(KeyEvent, u16, u16, u16)>,
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
            SortMode::SizeAsc => ("\u{f161}", "[SZ+]"),
            SortMode::SizeDesc => ("\u{f160}", "[SZ-]"),
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
        let (left_cap, right_cap) = selector_edge_spans(is_selected, spec, nerd_font_active);
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
    let footer_entries: &[(&'static str, &'static str)] = &[
        ("↑↓", "navigate"),
        ("Enter", "apply"),
        ("Tab", "switch tabs"),
        ("Esc", "close"),
    ];
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(footer_entries, theme_id, nerd_font_active)),
        sort_chunks[1],
    );
    footer_zones.extend(footer_shortcut_zones(footer_entries, sort_chunks[1], nerd_font_active));
}

pub fn render_themes_overlay(
    f: &mut Frame,
    tab_overlay_anchor: Rect,
    panel_tab: u8,
    theme_id: ThemeId,
    selected: usize,
    nerd_font: bool,
    nerd_focus: bool,
    color_mode: crate::FilenameColorMode,
    color_focus: bool,
    disable_clock: bool,
    clock_focus: bool,
    footer_zones: &mut Vec<(KeyEvent, u16, u16, u16)>,
) {
    let current = theme_spec(theme_id);
    let theme_w = tab_overlay_anchor.width;
    let theme_content_w = theme_w.saturating_sub(2) as usize;
    let theme_row_inner_w = theme_content_w.saturating_sub(2);
    const NERD_LABEL: &str = "Nerd Fonts";
    const COLOR_LABEL: &str = "Filename colors";
    const CLOCK_LABEL: &str = "Disable clock";
    // Width of the name column = longest theme name (display width), so the
    // checkboxes line up regardless of how long custom skin names are.
    let name_col_w = themes()
        .iter()
        .map(|t| UnicodeWidthStr::width(t.name))
        .max()
        .unwrap_or(0)
        .max(8)
        .max(UnicodeWidthStr::width(NERD_LABEL))
        .max(UnicodeWidthStr::width(COLOR_LABEL))
        .max(UnicodeWidthStr::width(CLOCK_LABEL));

    // Top toggle row: enable/disable Nerd Font glyphs (persisted to config).
    let nerd_base_style = if nerd_focus {
        Style::default().bg(current.bg_selected).fg(current.text_normal)
    } else {
        Style::default().fg(current.text_normal)
    };
    let nerd_pad = name_col_w.saturating_sub(UnicodeWidthStr::width(NERD_LABEL));
    let nerd_text = format!(
        " {}{} {}",
        NERD_LABEL,
        " ".repeat(nerd_pad),
        if nerd_font { "[x]" } else { "[ ]" }
    );
    let (nerd_left_cap, nerd_right_cap) = selector_edge_spans(nerd_focus, current, nerd_font);
    let mut nerd_row = vec![nerd_left_cap, Span::styled(nerd_text.clone(), nerd_base_style)];
    if nerd_focus {
        let used_w = UnicodeWidthStr::width(nerd_text.as_str());
        if theme_row_inner_w > used_w {
            nerd_row.push(Span::styled(
                " ".repeat(theme_row_inner_w - used_w),
                Style::default().bg(current.bg_selected),
            ));
        }
    }
    nerd_row.push(nerd_right_cap);

    // Second toggle row: filename-color mode (Full / Less / White), persisted.
    let color_base_style = if color_focus {
        Style::default().bg(current.bg_selected).fg(current.text_normal)
    } else {
        Style::default().fg(current.text_normal)
    };
    let color_pad = name_col_w.saturating_sub(UnicodeWidthStr::width(COLOR_LABEL));
    let color_text = format!(
        " {}{} [{}]",
        COLOR_LABEL,
        " ".repeat(color_pad),
        color_mode.label()
    );
    let (color_left_cap, color_right_cap) = selector_edge_spans(color_focus, current, nerd_font);
    let mut color_row = vec![color_left_cap, Span::styled(color_text.clone(), color_base_style)];
    if color_focus {
        let used_w = UnicodeWidthStr::width(color_text.as_str());
        if theme_row_inner_w > used_w {
            color_row.push(Span::styled(
                " ".repeat(theme_row_inner_w - used_w),
                Style::default().bg(current.bg_selected),
            ));
        }
    }
    color_row.push(color_right_cap);

    // Third toggle row: disable the header clock (show the disk-usage pill
    // instead), persisted to config.
    let clock_base_style = if clock_focus {
        Style::default().bg(current.bg_selected).fg(current.text_normal)
    } else {
        Style::default().fg(current.text_normal)
    };
    let clock_pad = name_col_w.saturating_sub(UnicodeWidthStr::width(CLOCK_LABEL));
    let clock_text = format!(
        " {}{} {}",
        CLOCK_LABEL,
        " ".repeat(clock_pad),
        if disable_clock { "[x]" } else { "[ ]" }
    );
    let (clock_left_cap, clock_right_cap) = selector_edge_spans(clock_focus, current, nerd_font);
    let mut clock_row = vec![clock_left_cap, Span::styled(clock_text.clone(), clock_base_style)];
    if clock_focus {
        let used_w = UnicodeWidthStr::width(clock_text.as_str());
        if theme_row_inner_w > used_w {
            clock_row.push(Span::styled(
                " ".repeat(theme_row_inner_w - used_w),
                Style::default().bg(current.bg_selected),
            ));
        }
    }
    clock_row.push(clock_right_cap);

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(nerd_row),
        Line::from(color_row),
        Line::from(clock_row),
        Line::from(""),
    ];
    // When a checkbox row holds focus, the theme list shows no cursor highlight
    // (so the selection fully moves to the checkbox, not duplicated in the list).
    let theme_list_focus = !nerd_focus && !color_focus && !clock_focus;
    for (idx, theme) in themes().iter().enumerate() {
        let is_selected = theme_list_focus && idx == selected;
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
        let (left_cap, right_cap) = selector_edge_spans(is_selected, current, nerd_font);
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
    // Scroll so the focused row stays visible when the theme list is taller
    // than the available height; draw a scrollbar on the right edge if it overflows.
    let total_lines = lines.len();
    let content_h = theme_chunks[0].height as usize;
    let cursor_line = if nerd_focus {
        1
    } else if color_focus {
        2
    } else if clock_focus {
        3
    } else {
        5 + selected
    };
    let max_offset = total_lines.saturating_sub(content_h);
    let offset = if cursor_line >= content_h {
        (cursor_line + 1 - content_h).min(max_offset)
    } else {
        0
    };
    let needs_scroll = total_lines > content_h && theme_chunks[0].width > 1;
    f.render_widget(Paragraph::new(lines).scroll((offset as u16, 0)), theme_chunks[0]);
    if needs_scroll {
        // Draw the scrollbar on the box's right border column (matching the
        // Integrations panel) so its track blends into the frame line.
        let sb_area = Rect::new(
            theme_area.x + theme_area.width.saturating_sub(1),
            theme_chunks[0].y,
            1,
            theme_chunks[0].height,
        );
        render_scrollbar_track(f, sb_area, total_lines, content_h, offset, max_offset, current);
    }
    let footer_entries: &[(&'static str, &'static str)] = &[
        ("↑↓", "select"),
        ("Enter/Space", "apply/toggle"),
        ("Esc", "close"),
    ];
    f.render_widget(
        Paragraph::new(shortcut_footer_lines(footer_entries, theme_id, nerd_font)),
        theme_chunks[1],
    );
    footer_zones.extend(footer_shortcut_zones(footer_entries, theme_chunks[1], nerd_font));
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

    #[test]
    fn footer_key_labels_map_to_events() {
        let code = |label| footer_key_label_to_event(label).map(|e| e.code);
        assert_eq!(code("Space"), Some(KeyCode::Char(' ')));
        assert_eq!(code("Tab"), Some(KeyCode::Tab));
        assert_eq!(code("Esc"), Some(KeyCode::Esc));
        assert_eq!(code("Enter"), Some(KeyCode::Enter));
        assert_eq!(code("Enter/0-9"), Some(KeyCode::Enter)); // combined label → primary key
        assert_eq!(code("Enter/Space"), Some(KeyCode::Enter));
        assert_eq!(code("Enter/→"), Some(KeyCode::Enter));
        assert_eq!(code("u/Delete"), Some(KeyCode::Char('u'))); // first segment wins
        assert_eq!(code("/"), Some(KeyCode::Char('/'))); // lone slash stays a key
        assert_eq!(code("c"), Some(KeyCode::Char('c')));
        assert_eq!(code("T"), Some(KeyCode::Char('T')));
        assert_eq!(code("↑↓"), None); // pure navigation hint is not clickable

        // Ctrl-prefixed labels map to the lowercase char with CONTROL set.
        let ctrl_t = footer_key_label_to_event("Ctrl+T").unwrap();
        assert_eq!(ctrl_t.code, KeyCode::Char('t'));
        assert!(ctrl_t.modifiers.contains(KeyModifiers::CONTROL));
        assert_eq!(code("Regex"), None); // informational label, not a key
    }

    #[test]
    fn footer_zones_track_pill_positions() {
        // Square (non-nerd) widths: key.width + desc.width, leading space, and
        // two-space separators. The blank first line pushes pills to y + 1.
        let entries: &[(&'static str, &'static str)] = &[
            ("Space", "toggle"), // width 11, x 1..12
            ("Enter", "apply"),  // sep → x 14..24
            ("↑↓", "nav"),       // width 5, skipped but still consumes layout
            ("Esc", "close"),    // sep → x 33..41
        ];
        let area = ratatui::layout::Rect::new(0, 0, 80, 2);
        let zones = footer_shortcut_zones(entries, area, false);
        let mapped: Vec<(KeyCode, u16, u16, u16)> =
            zones.iter().map(|(e, a, b, y)| (e.code, *a, *b, *y)).collect();
        assert_eq!(
            mapped,
            vec![
                (KeyCode::Char(' '), 1, 12, 1),
                (KeyCode::Enter, 14, 24, 1),
                (KeyCode::Esc, 33, 41, 1),
            ]
        );
    }
}
