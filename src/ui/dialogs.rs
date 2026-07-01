use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use std::path::{Path, PathBuf};
use crate::ui::theme::ThemeSpec;
use crate::OrganizeMove;

/// Build the spans for a single dialog button.
///
/// When a Nerd Font is active the button is drawn as a rounded "pill" using
/// Powerline half-circle caps (``/``); otherwise it falls back to the plain
/// space-padded label. Both variants occupy the same number of cells
/// (`label_len + 4`) so the button hit-test layout is identical either way.
fn dialog_button_spans(
    label: &str,
    focused: bool,
    focus_bg: Color,
    unfocused_fg: Color,
    nerd_font: bool,
    theme: &ThemeSpec,
) -> Vec<Span<'static>> {
    if nerd_font {
        let (text_fg, bg) = if focused {
            (Color::Rgb(20, 20, 30), focus_bg)
        } else {
            (unfocused_fg, theme.dialog_unfocus_bg)
        };
        let mut body = Style::default().fg(text_fg).bg(bg);
        if focused {
            body = body.add_modifier(Modifier::BOLD);
        }
        let cap = Style::default().fg(bg);
        vec![
            Span::styled("\u{e0b6}", cap),
            Span::styled(format!(" {} ", label), body),
            Span::styled("\u{e0b4}", cap),
        ]
    } else {
        let style = if focused {
            Style::default()
                .fg(Color::Rgb(20, 20, 30))
                .bg(focus_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(unfocused_fg)
        };
        vec![Span::styled(format!("  {}  ", label), style)]
    }
}

pub fn confirm_integration_install_msg_lines(
    key: &str,
    package: &str,
    brew_display: &str,
    brew_missing: bool,
) -> Vec<String> {
    let brew_command = Path::new(brew_display)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(brew_display);

    let mut msg_lines: Vec<String> = vec![
        String::new(),
        format!(" Integration: {}", key),
        format!(" Package:     {}", package),
        format!(" Command:     {} install {}", brew_command, package),
        String::new(),
    ];

    if brew_missing {
        msg_lines.push("Homebrew is not installed; setup guidance will be shown first.".to_string());
        msg_lines.push(String::new());
    }

    msg_lines
}

pub fn confirm_integration_install_dialog_area(area: Rect, msg_lines: &[String]) -> Rect {
    let content_w = msg_lines
        .iter()
        .map(|line| line.chars().count() as u16)
        .max()
        .unwrap_or(36);
    let content_h = msg_lines.len() as u16;
    let max_w = area.width.saturating_sub(4).max(1);
    let max_h = area.height.saturating_sub(4).max(1);
    let dialog_w = (content_w + 2).max(56).min(max_w);
    let dialog_h = (content_h + 4).max(8).min(max_h);
    Rect::new(
        (area.width.saturating_sub(dialog_w)) / 2,
        (area.height.saturating_sub(dialog_h)) / 2,
        dialog_w,
        dialog_h,
    )
}

pub fn confirm_ok_cancel_button_layout(area: Rect) -> Option<(Rect, u16, u16, u16, u16)> {
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    if inner.width == 0 || inner.height == 0 {
        return None;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let button_area = sections[1];

    let prefix_w = 2u16;
    let ok_w = "  OK  ".chars().count() as u16;
    let gap_w = 4u16;
    let cancel_w = "  Cancel  ".chars().count() as u16;
    let total_w = prefix_w + ok_w + gap_w + cancel_w;
    if button_area.width < total_w {
        return None;
    }

    let start_x = button_area.x + (button_area.width - total_w) / 2;
    let ok_start = start_x + prefix_w;
    let cancel_start = ok_start + ok_w + gap_w;
    Some((button_area, ok_start, ok_w, cancel_start, cancel_w))
}

pub fn render_confirm_integration_install_dialog(
    f: &mut Frame,
    msg_lines: &[String],
    confirm_area: Rect,
    button_focus: u8,
    nerd_font_active: bool,
    theme: &ThemeSpec,
) {
    f.render_widget(Clear, confirm_area);

    let title = if nerd_font_active {
        " \u{f01da} Install Integration "
    } else {
        " Install Integration "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_style(Style::default().fg(theme.text_normal));
    let inner = block.inner(confirm_area);
    let content_area = Rect::new(
        inner.x,
        inner.y,
        inner.width.saturating_sub(1),
        inner.height,
    );
    f.render_widget(block, confirm_area);

    let content_lines: Vec<Line<'static>> = msg_lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                return Line::from(Span::raw(" "));
            }

            let content = line.strip_prefix(' ').unwrap_or(line.as_str());
            if let Some((label, value)) = content.split_once(':') {
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        format!("{}:", label),
                        Style::default().fg(theme.accent_primary),
                    ),
                    Span::styled(value.to_string(), Style::default().fg(theme.text_normal)),
                ])
            } else {
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(content.to_string(), Style::default().fg(theme.accent_primary)),
                ])
            }
        })
        .collect();

    f.render_widget(
        Paragraph::new(content_lines).wrap(Wrap { trim: false }),
        content_area,
    );

    if let Some((button_area, _, _, _, _)) = confirm_ok_cancel_button_layout(confirm_area) {
        let ok_focused = button_focus == 0;
        let cancel_focused = !ok_focused;

        let mut button_spans: Vec<Span> = vec![Span::styled("  ", Style::default())];
        button_spans.extend(dialog_button_spans(
            "OK",
            ok_focused,
            theme.success,
            theme.text_dim,
            nerd_font_active,
            theme,
        ));
        button_spans.push(Span::styled("    ", Style::default()));
        button_spans.extend(dialog_button_spans(
            "Cancel",
            cancel_focused,
            theme.accent_primary,
            theme.text_dim,
            nerd_font_active,
            theme,
        ));
        let button_line = Line::from(button_spans);

        f.render_widget(
            Paragraph::new(button_line).alignment(Alignment::Center),
            button_area,
        );
    }
}

pub fn confirm_delete_title(file_count: usize, folder_count: usize) -> String {
    let plural = |count: usize, singular: &str, plural: &str| -> String {
        if count == 1 {
            singular.to_string()
        } else {
            plural.to_string()
        }
    };

    if file_count > 0 && folder_count > 0 {
        format!(
            " Delete {} {} and {} {}? ",
            file_count,
            plural(file_count, "file", "files"),
            folder_count,
            plural(folder_count, "folder", "folders")
        )
    } else if folder_count > 0 {
        format!(
            " Delete {} {}? ",
            folder_count,
            plural(folder_count, "folder", "folders")
        )
    } else {
        format!(
            " Delete {} {}? ",
            file_count,
            plural(file_count, "file", "files")
        )
    }
}

pub fn confirm_delete_dialog_area(area: Rect, title: &str) -> Rect {
    let content_w = title.chars().count().max(42) as u16;
    let content_h = area.height.saturating_sub(8).max(7);
    let max_w = area.width.saturating_sub(4).max(1);
    let max_h = area.height.saturating_sub(4).max(1);
    let dialog_w = (content_w + 2).max(48).min(max_w);
    let full_dialog_h = (content_h + 2).max(10).min(max_h);
    let dialog_h = (full_dialog_h / 2).max(8).min(max_h);
    Rect::new(
        (area.width.saturating_sub(dialog_w)) / 2,
        (area.height.saturating_sub(dialog_h)) / 2,
        dialog_w,
        dialog_h,
    )
}

pub fn confirm_delete_button_layout(area: Rect) -> Option<(Rect, u16, u16, u16, u16)> {
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    if inner.width == 0 || inner.height == 0 {
        return None;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let button_area = sections[1];

    let prefix_w = 2u16;
    let confirm_w = "  Confirm  ".chars().count() as u16;
    let gap_w = 4u16;
    let cancel_w = "  Cancel  ".chars().count() as u16;
    let total_w = prefix_w + confirm_w + gap_w + cancel_w;
    if button_area.width < total_w {
        return None;
    }

    let start_x = button_area.x + (button_area.width - total_w) / 2;
    let confirm_start = start_x + prefix_w;
    let cancel_start = confirm_start + confirm_w + gap_w;
    Some((button_area, confirm_start, confirm_w, cancel_start, cancel_w))
}

pub fn render_confirm_delete_buttons(
    f: &mut Frame,
    button_area: Rect,
    confirm_focused: bool,
    nerd_font_active: bool,
    theme: &ThemeSpec,
) {
    let cancel_focused = !confirm_focused;

    let mut button_spans: Vec<Span> = vec![Span::styled("  ", Style::default())];
    button_spans.extend(dialog_button_spans(
        "Confirm",
        confirm_focused,
        theme.error,
        theme.text_dim,
        nerd_font_active,
            theme,
    ));
    button_spans.push(Span::styled("    ", Style::default()));
    button_spans.extend(dialog_button_spans(
        "Cancel",
        cancel_focused,
        theme.accent_primary,
        theme.text_dim,
        nerd_font_active,
            theme,
    ));
    f.render_widget(
        Paragraph::new(Line::from(button_spans)).alignment(Alignment::Center),
        button_area,
    );
}

pub struct ConfirmDeleteRenderState {
    pub max_offset: u16,
    pub clamped_offset: u16,
}

/// The inputs to [`render_confirm_delete_dialog`] (everything except the frame,
/// the target area, and the per-row icon callback).
pub struct ConfirmDeleteView<'a> {
    pub title: &'a str,
    pub to_delete: &'a [PathBuf],
    pub scroll_offset: u16,
    pub confirm_focused: bool,
    pub show_icons: bool,
    pub nerd_font_active: bool,
    pub theme: &'a ThemeSpec,
}

pub fn render_confirm_delete_dialog<F>(
    f: &mut Frame,
    area: Rect,
    view: &ConfirmDeleteView,
    mut icon_for_path: F,
) -> ConfirmDeleteRenderState
where
    F: FnMut(&Path, bool) -> (String, Style),
{
    let &ConfirmDeleteView {
        title,
        to_delete,
        scroll_offset,
        confirm_focused,
        show_icons,
        nerd_font_active,
        theme,
    } = view;

    let confirm_area = confirm_delete_dialog_area(area, title);
    f.render_widget(Clear, confirm_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_style(Style::default().fg(theme.text_normal))
        .border_style(Style::default().fg(theme.error));
    let inner = block.inner(confirm_area);
    f.render_widget(block, confirm_area);

    if inner.width <= 2 || inner.height <= 2 {
        return ConfirmDeleteRenderState {
            max_offset: 0,
            clamped_offset: 0,
        };
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    let list_frame_area = sections[0];
    let list_inner = list_block.inner(list_frame_area);
    f.render_widget(list_block, list_frame_area);

    let needs_scroll = to_delete.len() > list_inner.height as usize;
    let can_draw_scrollbar = list_inner.width > 2 && needs_scroll;
    let list_area = list_inner;
    let visible_rows = list_area.height.max(1) as usize;
    let max_scroll = to_delete.len().saturating_sub(visible_rows);
    let offset = (scroll_offset as usize).min(max_scroll);

    let mut list_lines: Vec<Line> = Vec::new();
    if to_delete.is_empty() {
        list_lines.push(Line::from(Span::styled(
            "No selected item",
            Style::default().fg(theme.text_dim),
        )));
    } else {
        let row_name_max = list_area.width.saturating_sub(2) as usize;
        let truncate = |s: &str, max: usize| -> String {
            if max <= 1 {
                return "…".to_string();
            }
            let len = s.chars().count();
            if len <= max {
                return s.to_string();
            }
            s.chars().take(max - 1).collect::<String>() + "…"
        };

        for path in to_delete.iter().skip(offset).take(visible_rows) {
            let name = crate::util::classify::display_name(path.as_path());
            let path_is_symlink = path
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            let (icon_glyph, icon_style) = icon_for_path(path, path_is_symlink);
            let mut spans: Vec<Span> = Vec::new();
            if show_icons && !icon_glyph.is_empty() {
                spans.push(Span::styled(format!("{} ", icon_glyph), icon_style));
            }
            spans.push(Span::styled(
                truncate(&name, row_name_max.max(1)),
                Style::default().fg(theme.text_normal),
            ));
            list_lines.push(Line::from(spans));
        }
    }
    f.render_widget(Paragraph::new(list_lines), list_area);

    if can_draw_scrollbar {
        let sb_area = Rect::new(
            list_frame_area.x + list_frame_area.width.saturating_sub(1),
            list_inner.y,
            1,
            list_inner.height,
        );
        let track_h = sb_area.height as usize;
        if track_h > 0 {
            let mut sb_lines: Vec<Line> = Vec::with_capacity(track_h);
            let thumb_h = if to_delete.is_empty() {
                track_h
            } else {
                (visible_rows * track_h).div_ceil(to_delete.len())
                    .max(1)
                    .min(track_h)
            };
            let scroll_space = track_h.saturating_sub(thumb_h);
            let thumb_y = if max_scroll == 0 {
                0
            } else {
                (offset * scroll_space + (max_scroll / 2)) / max_scroll
            };

            for row in 0..track_h {
                let in_thumb = row >= thumb_y && row < thumb_y + thumb_h;
                let (ch, color) = if in_thumb {
                    ("┃", theme.divider)
                } else {
                    ("│", theme.border)
                };
                sb_lines.push(Line::from(Span::styled(ch, Style::default().fg(color))));
            }
            f.render_widget(Paragraph::new(sb_lines), sb_area);
        }
    }

    render_confirm_delete_buttons(f, sections[1], confirm_focused, nerd_font_active, theme);

    ConfirmDeleteRenderState {
        max_offset: max_scroll as u16,
        clamped_offset: offset as u16,
    }
}

pub fn render_confirm_delete_bookmark_dialog(
    f: &mut Frame,
    area: Rect,
    bookmark_idx: usize,
    bookmark_path: &str,
    from_env: bool,
    button_focus: u8,
    nerd_font_active: bool,
    theme: &ThemeSpec,
) {
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Bookmark: ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                format!("[{}]  {}", bookmark_idx, bookmark_path),
                Style::default().fg(theme.text_normal),
            ),
        ]),
    ];

    if from_env {
        lines.push(Line::from(vec![
            Span::styled("  Source:   ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                format!("$SB_BOOKMARK_{} (environment variable)", bookmark_idx),
                Style::default().fg(theme.warning),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  A deleted marker will be saved to config so this",
            Style::default().fg(theme.text_dim),
        )));
        lines.push(Line::from(Span::styled(
            "  bookmark stays hidden while the env var is set.",
            Style::default().fg(theme.text_dim),
        )));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  This bookmark will be removed from your config.",
            Style::default().fg(theme.text_dim),
        )));
    }
    lines.push(Line::from(""));

    let content_h = lines.len() as u16;
    let dialog_w = (bookmark_path.len() as u16 + 20).max(54).min(area.width.saturating_sub(4));
    let dialog_h = (content_h + 4).max(8).min(area.height.saturating_sub(4));
    let confirm_area = Rect::new(
        (area.width.saturating_sub(dialog_w)) / 2,
        (area.height.saturating_sub(dialog_h)) / 2,
        dialog_w,
        dialog_h,
    );
    f.render_widget(Clear, confirm_area);

    let title = if nerd_font_active {
        " \u{f1f8} Delete Bookmark "
    } else {
        " Delete Bookmark "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_style(Style::default().fg(theme.text_normal))
        .border_style(Style::default().fg(theme.error));
    let inner = block.inner(confirm_area);
    f.render_widget(block, confirm_area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), sections[0]);

    if let Some((button_area, _, _, _, _)) = confirm_ok_cancel_button_layout(confirm_area) {
        let delete_focused = button_focus == 0;
        let mut button_spans: Vec<Span> = vec![Span::styled("  ", Style::default())];
        button_spans.extend(dialog_button_spans(
            "Delete",
            delete_focused,
            theme.error,
            theme.text_dim,
            nerd_font_active,
            theme,
        ));
        button_spans.push(Span::styled("    ", Style::default()));
        button_spans.extend(dialog_button_spans(
            "Cancel",
            !delete_focused,
            theme.accent_primary,
            theme.text_dim,
            nerd_font_active,
            theme,
        ));
        f.render_widget(
            Paragraph::new(Line::from(button_spans)).alignment(Alignment::Center),
            button_area,
        );
    }
}

/// The inputs to [`render_organize_plan_dialog`] (everything except the frame
/// and target area).
pub struct OrganizePlanView<'a> {
    pub title: &'a str,
    pub folders: &'a [String],
    pub moves: &'a [OrganizeMove],
    pub scroll_offset: u16,
    pub confirm_focused: bool,
    pub nerd_font_active: bool,
    pub theme: &'a ThemeSpec,
}

/// Renders the AI-proposed organize plan for review: a bordered box with a
/// scrollable list (new folders as headers, moved entries indented beneath)
/// followed by Confirm/Cancel buttons. Mirrors
/// [`render_confirm_delete_dialog`]'s layout and scrolling.
pub fn render_organize_plan_dialog(f: &mut Frame, area: Rect, view: &OrganizePlanView) -> ConfirmDeleteRenderState {
    let &OrganizePlanView {
        title,
        folders,
        moves,
        scroll_offset,
        confirm_focused,
        nerd_font_active,
        theme,
    } = view;

    let confirm_area = confirm_delete_dialog_area(area, title);
    f.render_widget(Clear, confirm_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_style(Style::default().fg(theme.text_normal))
        .border_style(Style::default().fg(theme.accent_primary));
    let inner = block.inner(confirm_area);
    f.render_widget(block, confirm_area);

    if inner.width <= 2 || inner.height <= 2 {
        return ConfirmDeleteRenderState {
            max_offset: 0,
            clamped_offset: 0,
        };
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    let list_frame_area = sections[0];
    let list_inner = list_block.inner(list_frame_area);
    f.render_widget(list_block, list_frame_area);

    let mut rows: Vec<Line> = Vec::new();
    if moves.is_empty() {
        rows.push(Line::from(Span::styled(
            "No changes proposed",
            Style::default().fg(theme.text_dim),
        )));
    } else {
        let folder_style = Style::default()
            .fg(theme.accent_primary)
            .add_modifier(Modifier::BOLD);
        let name_style = Style::default().fg(theme.text_normal);
        for folder in folders {
            rows.push(Line::from(Span::styled(format!("📁 {}/", folder), folder_style)));
            for mv in moves.iter().filter(|m| &m.folder == folder) {
                rows.push(Line::from(Span::styled(format!("   {}", mv.name), name_style)));
            }
        }
        let orphaned: Vec<&OrganizeMove> = moves
            .iter()
            .filter(|m| !folders.iter().any(|f| f == &m.folder))
            .collect();
        if !orphaned.is_empty() {
            rows.push(Line::from(Span::styled("📁 (other)/", folder_style)));
            for mv in orphaned {
                rows.push(Line::from(Span::styled(format!("   {}", mv.name), name_style)));
            }
        }
    }

    let total_rows = rows.len();
    let needs_scroll = total_rows > list_inner.height as usize;
    let can_draw_scrollbar = list_inner.width > 2 && needs_scroll;
    let list_area = list_inner;
    let visible_rows = list_area.height.max(1) as usize;
    let max_scroll = total_rows.saturating_sub(visible_rows);
    let offset = (scroll_offset as usize).min(max_scroll);

    let visible: Vec<Line> = rows.into_iter().skip(offset).take(visible_rows).collect();
    f.render_widget(Paragraph::new(visible), list_area);

    if can_draw_scrollbar {
        let sb_area = Rect::new(
            list_frame_area.x + list_frame_area.width.saturating_sub(1),
            list_inner.y,
            1,
            list_inner.height,
        );
        let track_h = sb_area.height as usize;
        if track_h > 0 {
            let mut sb_lines: Vec<Line> = Vec::with_capacity(track_h);
            let thumb_h = (visible_rows * track_h).div_ceil(total_rows).max(1).min(track_h);
            let scroll_space = track_h.saturating_sub(thumb_h);
            let thumb_y = if max_scroll == 0 {
                0
            } else {
                (offset * scroll_space + (max_scroll / 2)) / max_scroll
            };

            for row in 0..track_h {
                let in_thumb = row >= thumb_y && row < thumb_y + thumb_h;
                let (ch, color) = if in_thumb {
                    ("┃", theme.divider)
                } else {
                    ("│", theme.border)
                };
                sb_lines.push(Line::from(Span::styled(ch, Style::default().fg(color))));
            }
            f.render_widget(Paragraph::new(sb_lines), sb_area);
        }
    }

    render_confirm_delete_buttons(f, sections[1], confirm_focused, nerd_font_active, theme);

    ConfirmDeleteRenderState {
        max_offset: max_scroll as u16,
        clamped_offset: offset as u16,
    }
}
