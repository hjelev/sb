use super::*;

/// Darken a color toward black (mirrors `ui::panels::darken`): used for the
/// free segment of the disk pill so it reads as a darker shade of the used fill.
pub(crate) fn darken_color(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * factor) as u8,
            (g as f32 * factor) as u8,
            (b as f32 * factor) as u8,
        ),
        _ => Color::Rgb(24, 24, 30),
    }
}

/// Build one span of the disk progress bar. The "used" portion gets the
/// threshold fill color as background with a dark foreground for contrast; the
/// "free" portion uses the dim panel background with normal-text foreground.
pub(crate) fn bar_span(
    text: String,
    in_used: bool,
    used_bg: Option<Color>,
    free_bg: Color,
    free_fg: Color,
) -> Span<'static> {
    match (in_used, used_bg) {
        (true, Some(bg)) => Span::styled(text, Style::default().bg(bg).fg(Color::Black)),
        _ => Span::styled(text, Style::default().bg(free_bg).fg(free_fg)),
    }
}

pub(crate) use crate::util::format::{truncate_display_width, truncate_with_ellipsis};

/// The display-style inputs a panel title needs: the active theme plus the
/// icon-rendering flags. Grouping them keeps `build_panel_title`'s signature
/// small (and avoids the `too_many_arguments` lint). Build one with
/// [`title_style`].
pub(crate) struct TitleStyle {
    theme: crate::ui::theme::ThemeSpec,
    show_icons: bool,
    nerd_font: bool,
    theme_id: crate::ui::theme::ThemeId,
}

pub(crate) fn title_style(app: &App, theme: crate::ui::theme::ThemeSpec) -> TitleStyle {
    TitleStyle {
        theme,
        show_icons: app.show_icons,
        nerd_font: app.nerd_font_active,
        theme_id: app.active_theme,
    }
}

pub(crate) fn build_panel_title(
    path: &std::path::Path,
    path_text: String,
    editing: bool,
    title_width: u16,
    style: &TitleStyle,
) -> Line<'static> {
    let is_symlink = crate::util::classify::is_symlink(path);
    let (folder_icon, folder_icon_style) = App::icon_for_path(
        path,
        style.show_icons,
        style.nerd_font,
        is_symlink,
        style.theme_id,
    );
    let title_inner_width = title_width.saturating_sub(2) as usize;
    let icon_width = if folder_icon.is_empty() {
        0
    } else {
        UnicodeWidthStr::width(folder_icon.as_str()) + 1
    };
    let prefix_width = 1 + icon_width;
    let text_max_width = title_inner_width.saturating_sub(prefix_width).max(1);
    let display_text = truncate_display_width(&path_text, text_max_width);
    let mut title_spans: Vec<Span> = Vec::new();
    title_spans.push(Span::raw(" "));
    if !folder_icon.is_empty() {
        title_spans.push(Span::styled(folder_icon, folder_icon_style));
        title_spans.push(Span::raw(" "));
    }
    if editing {
        title_spans.push(Span::styled(
            display_text.clone(),
            Style::default().fg(Color::Rgb(255, 220, 120)),
        ));
    } else {
        title_spans.push(Span::styled(
            display_text.clone(),
            Style::default().fg(style.theme.text_normal),
        ));
    }
    let used_width = prefix_width + UnicodeWidthStr::width(display_text.as_str());
    if used_width < title_inner_width {
        title_spans.push(Span::raw(" "));
    }
    Line::from(title_spans)
}

/// Rows `[start, end)` that a ratatui 0.29 `Table` of one-line rows shows in
/// `height` lines, given its previous scroll `offset` and `selected` row — a
/// port of `Table::get_row_bounds` for uniform rows. Lets callers build only
/// the visible `Row`s while keeping ratatui's scrolling behaviour. `None`
/// when nothing is drawn (ratatui then leaves the offset untouched).
pub(crate) fn visible_row_window(
    len: usize,
    offset: usize,
    selected: Option<usize>,
    height: usize,
) -> Option<(usize, usize)> {
    if len == 0 || height == 0 {
        return None;
    }
    let mut start = offset.min(len - 1);
    let mut end = (start + height).min(len);
    if let Some(selected) = selected.map(|s| s.min(len - 1)) {
        if selected >= end {
            end = selected + 1;
            start = start.max(end.saturating_sub(height));
        }
        if selected < start {
            start = selected;
            end = end.min(start + height);
        }
    }
    Some((start, end))
}

/// The selection fix-up ratatui applies to a `TableState` before drawing:
/// clamp to the last row, or clear it for an empty table. Applied to the
/// real state because the widget only ever sees the visible slice.
pub(crate) fn clamp_table_selection(state: &mut TableState, len: usize) {
    if len == 0 {
        state.select(None);
    } else if state.selected().is_some_and(|s| s >= len) {
        state.select(Some(len - 1));
    }
}

/// Draw a `Table` holding only the rows of `window` (from
/// [`visible_row_window`]) and record the window start as `state`'s scroll
/// offset, so mouse hit-testing and overlays that read
/// `state.offset()`/`state.selected()` see the same values as before.
pub(crate) fn render_table_window(
    f: &mut Frame,
    table: Table,
    area: Rect,
    state: &mut TableState,
    window: Option<(usize, usize)>,
) {
    let Some((start, _)) = window else {
        f.render_widget(table, area);
        return;
    };
    let mut view = TableState::default().with_selected(state.selected().map(|s| s - start));
    f.render_stateful_widget(table, area, &mut view);
    *state.offset_mut() = start;
}

#[cfg(test)]
mod visible_row_window_tests {
    use super::visible_row_window;

    /// Reference: ratatui's own bounds, computed by rendering a real table.
    fn ratatui_bounds(
        len: usize,
        offset: usize,
        selected: Option<usize>,
        height: u16,
    ) -> (usize, usize) {
        use ratatui::{buffer::Buffer, layout::Rect, widgets::*};
        let rows: Vec<Row> = (0..len).map(|i| Row::new(vec![i.to_string()])).collect();
        let table = Table::new(rows, [ratatui::layout::Constraint::Min(0)]);
        let mut state = TableState::default()
            .with_offset(offset)
            .with_selected(selected);
        let area = Rect::new(0, 0, 10, height);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(table, area, &mut buf, &mut state);
        let start = state.offset();
        (start, (start + height as usize).min(len))
    }

    #[test]
    fn matches_ratatui_scrolling() {
        for len in [1usize, 2, 5, 12, 40] {
            for height in [1u16, 3, 10] {
                for offset in [0usize, 1, 4, 11, 39, 60] {
                    for selected in [None, Some(0), Some(3), Some(11), Some(39), Some(99)] {
                        let ours =
                            visible_row_window(len, offset, selected, height as usize).unwrap();
                        let theirs = ratatui_bounds(len, offset, selected, height);
                        assert_eq!(
                            ours.0, theirs.0,
                            "start: len={len} h={height} off={offset} sel={selected:?}"
                        );
                        // ratatui may show fewer rows than `height` when the
                        // offset sits near the end; our end is the row bound.
                        assert!(ours.1 <= len && ours.1 - ours.0 <= height as usize);
                        if let Some(sel) = selected {
                            let sel = sel.min(len - 1);
                            assert!(ours.0 <= sel && sel < ours.1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn empty_or_zero_height_draws_nothing() {
        assert_eq!(visible_row_window(0, 0, Some(0), 10), None);
        assert_eq!(visible_row_window(10, 3, Some(0), 0), None);
    }
}
