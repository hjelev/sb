use crate::integration::rows::IntegrationRow;
use crate::ui::theme::{theme_spec, ThemeId};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph},
};
use unicode_width::UnicodeWidthStr;

pub(crate) const PANEL_TABS: &[(&str, u8)] = &[
    (" Help ", 0),
    (" Search ", 1),
    (" Bookmarks ", 2),
    (" Remote Mounts ", 3),
    (" Sorting ", 4),
    (" Integrations ", 5),
    (" Themes ", 6),
    (" Settings ", 7),
    (" Shortcuts ", 8),
    (" Plugins ", 9),
];

// Scroll indicators shown when the tab bar is wider than the available space.
const TAB_MORE_LEFT: &str = "‹";
const TAB_MORE_RIGHT: &str = "›";

/// Rendered width of a single tab label (its full padded text).
pub(crate) fn tab_label_width(index: usize) -> usize {
    PANEL_TABS[index].0.chars().count()
}

/// Decide which contiguous run of tabs is visible for the given `active` tab and
/// available title `avail` width. Returns `(lo, hi, more_left, more_right)` where
/// `lo..=hi` is the visible range (always including `active`) and the booleans
/// indicate hidden tabs beyond each edge (drawn as `‹` / `›`). When everything
/// fits, the full range is returned and no indicators are shown.
pub(crate) fn visible_tab_window(active: usize, avail: usize) -> (usize, usize, bool, bool) {
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
    for (i, &(label, idx)) in PANEL_TABS.iter().enumerate().take(hi + 1).skip(lo) {
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
pub(crate) fn render_overlay_block(
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

/// Render a vertical scrollbar track into `sb_area` with the overlay-panel
/// colors (shared geometry lives in [`crate::ui::scrollbar`]).
pub(crate) fn render_scrollbar_track(
    f: &mut Frame,
    sb_area: Rect,
    total_rows: usize,
    visible_rows: usize,
    scroll_offset: usize,
    spec: &crate::ui::theme::ThemeSpec,
) {
    crate::ui::scrollbar::render_scrollbar_track(
        f,
        sb_area,
        total_rows,
        visible_rows,
        scroll_offset,
        spec.accent_primary,
        spec.divider,
    );
}

/// Prepend a single space to each line (for left-padding inside overlay panels).
pub(crate) fn indent_lines<'a>(lines: &[Line<'a>]) -> Vec<Line<'a>> {
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

    for (i, &(_, idx)) in PANEL_TABS.iter().enumerate().take(hi + 1).skip(lo) {
        if i > lo {
            if relative_x == cursor {
                return None;
            }
            cursor = cursor.saturating_add(1);
        }

        let width = tab_label_width(i) as u16;
        if relative_x >= cursor && relative_x < cursor.saturating_add(width) {
            return Some(idx);
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
    let key_fg = crate::ui::palette::readable_fg(key_bg, Color::Black, spec.text_normal);
    let label_fg = crate::ui::palette::readable_fg(label_bg, Color::Black, spec.text_normal);
    let key_style = Style::default()
        .fg(key_fg)
        .bg(key_bg)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(label_fg).bg(label_bg);
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
pub(crate) fn footer_key_label_to_event(label: &str) -> Option<KeyEvent> {
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

pub(crate) fn selector_edge_spans(is_selected: bool, spec: &crate::ui::theme::ThemeSpec, nerd_font: bool) -> (Span<'static>, Span<'static>) {
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

pub struct IntegrationsOverlayState<'a> {
    pub integrations: &'a [IntegrationRow],
    pub integration_selected: usize,
    pub search_active: bool,
    pub search_query: &'a str,
    pub show_icons: bool,
}


// Overlay renderers live in panels_overlays.rs; re-exported so `ui::panels::*` paths hold.
pub use super::panels_overlays::*;
