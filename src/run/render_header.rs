use super::*;

pub(crate) fn render_header(f: &mut Frame, app: &mut App, ctx: &RenderCtx, user: &str, hostname: &str) {
    let active_theme = ctx.theme;
    let chunks = [ctx.main, ctx.footer];
    #[allow(unused_variables)]
    let header_reserved_rows = ctx.header_rows;
    let scrollbar_visible_in_main = ctx.scrollbar_in_main;

    // --- Header ---
    let header_identity = app.current_header_identity(user, hostname);
    let current_display_path = if app.mode == AppMode::PathEditing {
        app.input_buffer.clone()
    } else {
        app.current_dir_display_path_with_filter()
    };
    let normal_view = !app.is_preview_mode() && !app.is_dual_panel_mode();
    // In normal mode, nudge the folder icon one character to the right.
    let header_sep = if app.nerd_font_active {
        if normal_view { " \u{f0256} " } else { "\u{f0256} " }
    } else {
        " » "
    };
    let os_icon_glyph: Option<&'static str> = if app.nerd_font_active {
        // Use the remote OS icon if we're inside an SSH/rclone mount
        app.remote.ssh_mounts.iter().rfind(|m| app.left.dir.starts_with(&m.mount_path))
            .and_then(|m| m.remote_os_icon.map(|(glyph, _)| glyph))
            .or_else(|| app.os_icon.map(|(glyph, _)| glyph))
    } else {
        None
    };
    let os_icon_color = ui::theme::theme_spec(app.active_theme).icon_os;
    let mut middle_spans: Vec<Span> = Vec::new();
    let os_icon_width: u16;
    if let (Some(glyph), Some((left_identity, right_identity))) =
        (os_icon_glyph, header_identity.split_once('@'))
    {
        // Pad icon with a space on each side so the glyph has breathing room
        // and renders at a readable size across different terminals.
        let icon_text = format!("{} ", glyph);
        os_icon_width = UnicodeWidthStr::width(icon_text.as_str()) as u16;
        middle_spans.push(Span::raw(left_identity.to_string()));
        middle_spans.push(Span::styled(icon_text, Style::default().fg(os_icon_color)));
        middle_spans.push(Span::raw(right_identity.to_string()));
    } else {
        // Fallback: prepend icon (with trailing space) then identity
        let os_icon_span: Option<Span> = os_icon_glyph.map(|glyph| {
            Span::styled(format!("{} ", glyph), Style::default().fg(os_icon_color))
        });
        os_icon_width = os_icon_span
            .as_ref()
            .map(|s| UnicodeWidthStr::width(s.content.as_ref()) as u16)
            .unwrap_or(0);
        if let Some(icon_span) = os_icon_span {
            middle_spans.push(icon_span);
        }
        if let Some((left_identity, right_identity)) = header_identity.split_once('@') {
            middle_spans.push(Span::raw(left_identity.to_string()));
            middle_spans.push(Span::styled("@", Style::default().fg(active_theme.text_dim)));
            middle_spans.push(Span::raw(right_identity.to_string()));
        } else {
            middle_spans.push(Span::raw(header_identity.as_str()));
        }
    }

    let header_sep_span = if app.nerd_font_active {
        Span::styled(
            header_sep,
            Style::default()
                .fg(active_theme.accent_primary)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(header_sep)
    };
    let mut left_spans: Vec<Span> = if app.is_preview_mode() || app.is_dual_panel_mode() {
        vec![]
    } else {
        vec![
            header_sep_span,
            if app.mode == AppMode::PathEditing {
                Span::styled(current_display_path.as_str(), Style::default().fg(active_theme.warning))
            } else {
                Span::raw(current_display_path.as_str())
            },
        ]
    };
    if app.integration_enabled("git")
        && let Some((branch, is_dirty, tag_info)) = app.cached_git_info_for_current_dir() {
            let branch_style = Style::default().fg(active_theme.git_branch);
            left_spans.push(Span::styled(" (", branch_style));
            left_spans.push(Span::styled(branch, branch_style));
            if is_dirty {
                left_spans.push(Span::styled("*", Style::default().fg(active_theme.text_normal)));
            }
            if let Some((tag_name, ahead)) = tag_info {
                let at_style = Style::default().fg(active_theme.text_dim);
                let tag_style = Style::default().fg(active_theme.git_tag);
                let tag_text = if ahead > 0 {
                    format!("{}+{}", tag_name, ahead)
                } else {
                    tag_name.to_string()
                };
                left_spans.push(Span::styled("@", at_style));
                left_spans.push(Span::styled(tag_text, tag_style));
            }
            left_spans.push(Span::styled(")", branch_style));
        }
    let mut header_right_is_clock = false;
    let header_right = if let Some(disk_info) = app.current_dir_total_size_header_info() {
        let icon_style = Style::default().fg(active_theme.accent_primary);
        let text_style = Style::default().fg(active_theme.text_normal);
        let mut spans: Vec<Span> = Vec::new();

        // Folder-size prefix: recolor the folder glyph, plain text otherwise.
        let mut text_buf = String::new();
        for ch in disk_info.folder_segment.chars() {
            if ch == '\u{f10b7}' {
                if !text_buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut text_buf), text_style));
                }
                spans.push(Span::styled(ch.to_string(), icon_style));
            } else {
                text_buf.push(ch);
            }
        }
        if !text_buf.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut text_buf), text_style));
        }

        // Disk label rendered as a two-tone pill progress bar (same look as the
        // footer shortcut pills): the left `used_fraction` of the width is filled
        // with a threshold color (green/amber/red), the remainder uses a darker
        // shade of that color. With Nerd Fonts the ends get rounded Powerline
        // caps; without, it degrades to a square two-tone block.
        let bar = &disk_info.disk_segment;
        let bar_width = UnicodeWidthStr::width(bar.as_str());
        let used_color = disk_info.used_fraction.map(|f| {
            if f >= 0.90 {
                active_theme.error
            } else if f >= 0.70 {
                active_theme.warning
            } else {
                active_theme.success
            }
        });
        let used_bg = used_color;
        let free_bg = used_color
            .map(|c| darken_color(c, 0.45))
            .unwrap_or(active_theme.bg_inactive_panel);
        let fill_cols = disk_info
            .used_fraction
            .map(|f| ((bar_width as f64) * f).round() as usize)
            .unwrap_or(0)
            .min(bar_width);

        // Cap colors match the color of the cell they round into.
        let first_color = if fill_cols > 0 { used_bg.unwrap_or(free_bg) } else { free_bg };
        let last_color = if fill_cols >= bar_width { used_bg.unwrap_or(free_bg) } else { free_bg };

        // Split the label into the used (filled) and free runs by column.
        let mut col = 0usize;
        let mut used_text = String::new();
        let mut free_text = String::new();
        for ch in bar.chars() {
            if col < fill_cols {
                used_text.push(ch);
            } else {
                free_text.push(ch);
            }
            col += UnicodeWidthStr::width(ch.to_string().as_str());
        }

        if app.nerd_font_active {
            spans.push(Span::styled("\u{e0b6}", Style::default().fg(first_color)));
        }
        if !used_text.is_empty() {
            spans.push(bar_span(used_text, true, used_bg, free_bg, active_theme.text_normal));
        }
        if !free_text.is_empty() {
            spans.push(bar_span(free_text, false, used_bg, free_bg, active_theme.text_normal));
        }
        if app.nerd_font_active {
            spans.push(Span::styled("\u{e0b4}", Style::default().fg(last_color)));
        }
        // One trailing space so the pill isn't flush against the right edge.
        spans.push(Span::raw(" "));

        Some(Line::from(spans))
    } else if !app.size.folder_size_enabled {
        header_right_is_clock = true;
        Some(Line::from(vec![
            Span::styled(app.header_clock.text.clone(), Style::default().fg(active_theme.text_normal)),
        ]))
    } else {
        None
    };

    let left_content_width: u16 = left_spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()) as u16)
        .sum();
    let middle_content_width = os_icon_width + (UnicodeWidthStr::width(header_identity.as_str()) as u16);

    let min_left_width: u16 = 12;
    let min_middle_width: u16 = 8;
    let max_middle_width: u16 = 24;
    let left_required_width = min_left_width;
    let left_preferred_width = left_content_width.saturating_add(1).max(min_left_width);

    let mut show_right = header_right.is_some();
    let mut right_width = header_right
        .as_ref()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()) as u16)
                .sum::<u16>()
                .saturating_add(1)
        })
        .unwrap_or(0)
        .min(chunks[0].width);
    let right_required_width = right_width;
    if right_required_width == 0 {
        show_right = false;
    }
    if show_right {
        let min_total_with_right = min_left_width
            .saturating_add(min_middle_width)
            .saturating_add(right_required_width);
        if chunks[0].width < min_total_with_right {
            show_right = false;
        }
    }
    if !show_right {
        right_width = 0;
    }

    let total_width = chunks[0].width;
    let desired_middle_width = middle_content_width
        .saturating_add(1)
        .min(max_middle_width);
    let mut middle_width = desired_middle_width
        .max(min_middle_width)
        .min(total_width.saturating_sub(2));

    let centered_middle_start = total_width.saturating_sub(middle_width) / 2;
    let mut middle_start = centered_middle_start;
    let mut left_width = middle_start;

    // Left (path+git) priority: if left area is too small, first hide right, then shrink middle.
    if left_width < left_required_width && show_right {
        show_right = false;
        right_width = 0;
    }
    if show_right {
        let right_start = total_width.saturating_sub(right_width);
        let middle_end = middle_start.saturating_add(middle_width);
        if middle_end > right_start {
            show_right = false;
            right_width = 0;
        }
    }
    let max_middle_start = if show_right {
        total_width
            .saturating_sub(right_width)
            .saturating_sub(middle_width)
    } else {
        total_width.saturating_sub(middle_width)
    };
    if left_width < left_preferred_width {
        middle_start = left_preferred_width.min(max_middle_start);
        left_width = middle_start;
    }
    if !show_right {
        middle_start = middle_start.max(centered_middle_start);
        left_width = middle_start;
    }
    if left_width < left_required_width {
        let reserved_right = if show_right { right_width } else { 0 };
        let max_middle_for_left = total_width
            .saturating_sub(left_required_width)
            .saturating_sub(reserved_right);
        if max_middle_for_left >= min_middle_width {
            middle_width = middle_width.min(max_middle_for_left);
            middle_start = if show_right {
                left_required_width.min(
                    total_width
                        .saturating_sub(right_width)
                        .saturating_sub(middle_width),
                )
            } else {
                left_required_width.min(total_width.saturating_sub(middle_width))
            };
            left_width = middle_start;
        }
    }

    let left_rect = Rect::new(chunks[0].x, chunks[0].y, left_width, 1);
    let middle_rect_width = if show_right {
        middle_width
    } else {
        total_width.saturating_sub(middle_start)
    };
    let middle_rect = Rect::new(chunks[0].x + middle_start, chunks[0].y, middle_rect_width, 1);

    if left_rect.width > 0 {
        f.render_widget(
            Paragraph::new(Line::from(left_spans)).alignment(Alignment::Left),
            left_rect,
        );
    }
    if middle_rect.width > 0 {
        let middle_alignment = if show_right { Alignment::Center } else { Alignment::Right };
        f.render_widget(
            Paragraph::new(Line::from(middle_spans)).alignment(middle_alignment),
            middle_rect,
        );
    }
    if show_right
        && let Some(header_right_line) = header_right {
            let mut scrollbar_offset = if scrollbar_visible_in_main { 1 } else { 0 };
            // Nudge the clock left by a per-view amount.
            if header_right_is_clock {
                if normal_view {
                    scrollbar_offset += 1;
                } else if app.is_dual_panel_mode() {
                    scrollbar_offset += 2;
                } else if app.is_preview_mode() {
                    scrollbar_offset += 1;
                }
            }
            let right_rect = Rect::new(
                chunks[0].x + total_width.saturating_sub(right_width).saturating_sub(scrollbar_offset),
                chunks[0].y,
                right_width,
                1,
            );
            if right_rect.width > 0 {
                f.render_widget(
                    Paragraph::new(header_right_line).alignment(Alignment::Right),
                    right_rect,
                );
            }
        }
    if app.mode == AppMode::PathEditing {
        let sep_len = UnicodeWidthStr::width(header_sep) as u16;
        app.clamp_input_cursor();
        let left_end_x = chunks[0]
            .x
            .saturating_add(left_width.saturating_sub(1));
        let left_x = chunks[0].x;
        let cursor_x = (left_x + sep_len + app.input_cursor as u16)
            .min(left_end_x);
        let cursor_y = chunks[0].y;
        f.set_cursor(cursor_x, cursor_y);
    }
    f.render_widget(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(active_theme.border)),
        Rect::new(chunks[0].x, chunks[0].y + 1, chunks[0].width, 1),
    );

}

/// Render the single-line folder-filter input box at the top of a panel's body
/// area. Returns the body area shrunk by the consumed row so the listing can be
/// laid out below it. When `focused`, positions the terminal cursor in the box.
pub(crate) fn render_folder_filter_box(
    f: &mut Frame,
    app: &App,
    body_area: Rect,
    theme: crate::ui::theme::ThemeSpec,
    focused: bool,
) -> Rect {
    if body_area.height <= 1 || body_area.width == 0 {
        return body_area;
    }
    let box_area = Rect::new(body_area.x, body_area.y, body_area.width, 1);
    // Nerd Font: magnifying-glass glyph; otherwise a plain "/" to match the key.
    let prefix = if app.nerd_font_active { " \u{f0349} " } else { " / " };
    let prefix_style = Style::default()
        .fg(theme.accent_primary)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(theme.warning);
    let line = Line::from(vec![
        Span::styled(prefix, prefix_style),
        Span::styled(app.input_buffer.as_str(), text_style),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme.bg_panel)),
        box_area,
    );
    if focused {
        let prefix_w = UnicodeWidthStr::width(prefix) as u16;
        let cursor_x = box_area.x + prefix_w + app.input_cursor as u16;
        let max_x = box_area.x + box_area.width.saturating_sub(1);
        f.set_cursor(cursor_x.min(max_x), box_area.y);
    }
    Rect::new(
        body_area.x,
        body_area.y + 1,
        body_area.width,
        body_area.height - 1,
    )
}

