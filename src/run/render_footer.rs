use super::*;

pub(crate) fn render_footer(f: &mut Frame, app: &mut App, ctx: &RenderCtx) {
    let active_theme = ctx.theme;
    let chunks = [ctx.main, ctx.footer];
    let header_reserved_rows = ctx.header_rows;

    let spec = ui::theme::theme_spec(app.active_theme);
    let nf = app.nerd_font_active;

    // --- Footer ---
    let (total_entries, selected_ordinal) = if app.is_dual_panel_mode()
        && app.active_panel == crate::DualPanelSide::Right
    {
        let total = app.right.entries.len();
        let ord = if total == 0 { 0 } else { app.right.selected_index.min(total - 1) + 1 };
        (total, ord)
    } else {
        let total = app.entries.len();
        let ord = if total == 0 { 0 } else { app.selected_index.min(total - 1) + 1 };
        (total, ord)
    };
    let ordinal_str = selected_ordinal.to_string();
    let total_str = total_entries.to_string();

    let width = chunks[1].width as usize;

    // Counter rendered as a two-tone pill matching the footer shortcuts: the
    // current index is the (lighter) key segment, the total is the (darker)
    // label segment. No `/` separator.
    let mut left_spans: Vec<Span> = ui::panels::shortcut_spans(&ordinal_str, &total_str, nf, spec);
    let mut left_len = ui::panels::shortcut_width(&ordinal_str, &total_str, nf);
    if !app.clipboard.is_empty() {
        left_len += format!(" │ Clipboard:{}", app.clipboard.len()).chars().count();
        left_spans.push(Span::styled(" │ ", Style::default().fg(active_theme.text_dim)));
        left_spans.push(Span::styled("Clipboard", Style::default().fg(active_theme.text_dim)));
        left_spans.push(Span::styled(":", Style::default().fg(active_theme.text_dim)));
        left_spans.push(Span::styled(app.clipboard.len().to_string(), Style::default().fg(active_theme.text_normal)));
    }

    // Footer shortcuts: pill-styled keys (nerd font) or plain keys,
    // each followed by its description. No `:` separator.
    const FOOTER_SHORTCUTS: &[(&str, &str)] = &[
        ("c", "Copy"),
        ("v", "paste"),
        ("m", "Move"),
        ("r", "Rename"),
        ("w", "Web"),
        ("d", "Del"),
        ("e", "Edit"),
        ("s", "Size"),
        ("o", "Open-GUI"),
        ("f", "Find"),
        ("`", "Mode"),
        ("h", "Help"),
        ("q", "Quit"),
    ];
    let sep_w = 1usize; // single space between shortcuts
    let avail_right = width.saturating_sub(left_len + 1);

    // Prefer the tail (rightmost) shortcuts when space is tight.
    let mut start = FOOTER_SHORTCUTS.len();
    let mut right_len = 0usize;
    for i in (0..FOOTER_SHORTCUTS.len()).rev() {
        let (k, d) = FOOTER_SHORTCUTS[i];
        let w = ui::panels::shortcut_width(k, d, nf)
            + if start == FOOTER_SHORTCUTS.len() { 0 } else { sep_w };
        if right_len + w <= avail_right {
            right_len += w;
            start = i;
        } else {
            break;
        }
    }

    // Register clickable hit-zones for the footer pills. The vec is cleared
    // once per frame in the draw closure (overlay footers append earlier), so
    // here we only append. Only register in Browsing mode (preview/dual panel
    // are still Browsing) so the main footer pills are inert under an overlay.
    let zones_enabled = app.mode == crate::AppMode::Browsing;
    // The shortcut text sits below the optional top border (normal mode) or on
    // the single footer row (preview/dual panel mode).
    let zone_y = if app.is_preview_mode() || app.is_dual_panel_mode() {
        chunks[1].y
    } else {
        chunks[1].y + 1
    };
    // The right-aligned block begins after the left segment and gap fill.
    let mut zone_x = (chunks[1].x as usize + width.saturating_sub(right_len)) as u16;

    let mut right_spans: Vec<Span> = Vec::new();
    for (idx, (k, d)) in FOOTER_SHORTCUTS[start..].iter().enumerate() {
        if idx > 0 {
            right_spans.push(Span::raw(" "));
            zone_x = zone_x.saturating_add(sep_w as u16);
        }
        let pill_w = ui::panels::shortcut_width(k, d, nf) as u16;
        if zones_enabled && let Some(key) = k.chars().next() {
            let event = KeyEvent::new(
                crossterm::event::KeyCode::Char(key),
                crossterm::event::KeyModifiers::NONE,
            );
            app.footer_shortcut_zones.push((event, zone_x, zone_x + pill_w, zone_y));
        }
        zone_x = zone_x.saturating_add(pill_w);
        right_spans.extend(ui::panels::shortcut_spans(k, d, nf, spec));
    }

    let gap = " ".repeat(width.saturating_sub(left_len + right_len));

    let mut status_spans: Vec<Span> = left_spans;
    status_spans.push(Span::raw(gap));
    status_spans.extend(right_spans);
    let status = Line::from(status_spans);
    let footer_block = if app.is_preview_mode() || app.is_dual_panel_mode() {
        Block::default().borders(Borders::NONE)
    } else {
        Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(active_theme.border))
    };
    f.render_widget(Paragraph::new(status).block(footer_block), chunks[1]);
    if !app.is_preview_mode() && !app.is_dual_panel_mode() {
        let selected_total_status = if app.copy.rx.is_none() && app.archive.rx.is_none() {
            app.selected_total_size_status()
        } else {
            None
        };

        let selected_total_is_shown = selected_total_status.is_some();
        let status_line_message = selected_total_status.or_else(|| {
            if app.status_message.is_empty() {
                None
            } else {
                Some(app.status_message.clone())
            }
        });

        if let Some(status_text) = status_line_message {
            let msg_area = Rect::new(chunks[1].x, chunks[1].y, chunks[1].width, 1);
            let is_error = crate::ui::status::is_error_message(&status_text);
            let msg_style = if selected_total_is_shown {
                Style::default().fg(active_theme.git_added)
            } else if app.copy.rx.is_some() || app.archive.rx.is_some() {
                Style::default().fg(active_theme.git_modified)
            } else if is_error {
                Style::default().fg(active_theme.git_deleted)
            } else {
                Style::default().fg(active_theme.text_normal)
            };
            let decorated = app.decorate_footer_message(&status_text);
            let message = decorated.as_str();
            let core = format!("─── {} ", message);
            let core_len = core.chars().count();
            let width = msg_area.width as usize;
            let line_msg = if core_len >= width {
                core.chars().take(width).collect::<String>()
            } else {
                let remaining = width - core_len;
                format!("{}{}", core, "─".repeat(remaining))
            };
            f.render_widget(
                Paragraph::new(line_msg).style(msg_style),
                msg_area,
            );
        }
    }

    // Render scrollbar corners on top of all other elements only if no overlay is active
    if app.mode_shows_main_scrollbar() && !app.entries.is_empty() {
        let table_area = Rect::new(
            chunks[0].x,
            chunks[0].y + header_reserved_rows,
            chunks[0].width,
            chunks[0].height.saturating_sub(header_reserved_rows),
        );
        if app.is_preview_mode() || app.is_dual_panel_mode() {
            // In split preview mode, extra corner overlays can clash with the
            // rounded pane border; skip the synthetic scrollbar corners.
        } else {
            let can_draw_scrollbar = table_area.width > 2 && app.entries.len() > table_area.height as usize;
            ui::scrollbar::render_scrollbar_corners(f, table_area, can_draw_scrollbar, active_theme.border);
        }
    }
}

