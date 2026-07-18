//! Scrollbar + preview-pane rendering. Split from render_table.rs.
use super::*;
use crate::ui::list_metrics::*;

pub(crate) fn render_scrollbar_and_preview(f: &mut Frame, app: &mut App, ctx: &RenderCtx, tl: &TableLayout) {
    let active_theme = ctx.theme;
    let chunks = [ctx.main, ctx.footer];
    let list_frame_area = tl.list_frame_area;
    let preview_frame_area = tl.preview_frame_area;
    let table_area = tl.table_area;
    let can_draw_scrollbar = tl.can_draw_scrollbar;
    let list_area = tl.list_area;
    let note_style = Style::default().fg(ctx.theme.text_dim);
    let tree_style = Style::default().fg(active_theme.text_dim);
    let right_selection_style = if app.is_dual_panel_mode() {
        match app.active_panel {
            crate::DualPanelSide::Left => Style::default().bg(ctx.theme.bg_inactive_panel),
            crate::DualPanelSide::Right => Style::default().bg(ctx.theme.bg_selected),
        }
    } else {
        Style::default().bg(ctx.theme.bg_selected)
    };

    // --- Bottom divider border ---
    let bottom_border_y = table_area.y + table_area.height;
    if !app.is_preview_mode() && !app.is_dual_panel_mode() && app.mode_shows_main_scrollbar() && bottom_border_y < chunks[0].y + chunks[0].height {
        f.render_widget(Block::default().borders(Borders::TOP).border_type(BorderType::Rounded).border_style(Style::default().fg(active_theme.border)), 
            Rect::new(chunks[0].x, bottom_border_y, chunks[0].width, 1));
    }

    if can_draw_scrollbar {
        let sb_area = Rect::new(
            if app.is_preview_mode() || app.is_dual_panel_mode() {
                list_frame_area.x + list_frame_area.width.saturating_sub(1)
            } else {
                table_area.x + table_area.width.saturating_sub(1)
            },
            table_area.y,
            1,
            table_area.height,
        );
        let visible_rows = list_area.height.max(1) as usize;
        ui::scrollbar::render_scrollbar_track(
            f,
            sb_area,
            app.left.entries.len(),
            visible_rows,
            app.left.table_state.offset(),
            active_theme.divider,
            active_theme.border,
        );
    }

    app.preview.native_area = None;
    if let Some(preview_area) = preview_frame_area {
        if app.is_preview_mode() {
        let title_path = app
            .preview.target_path
            .clone()
            .or_else(|| app.left.entries.get(app.left.selected_index).map(|e| e.path()));
        let preview_title = if let Some(path) = title_path {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|n| !n.is_empty())
                .unwrap_or("Preview")
                .to_string();
            let is_symlink = crate::util::classify::is_symlink(&path);
            let (icon_glyph, icon_style) = App::icon_for_path(
                &path,
                app.show_icons,
                app.nerd_font_active,
                is_symlink,
                app.active_theme,
            );
            let title_width = preview_area.width.saturating_sub(2) as usize;
            let icon_width = if icon_glyph.is_empty() {
                0
            } else {
                UnicodeWidthStr::width(icon_glyph.as_str()) + 1
            };
            let prefix_width = 1 + icon_width;
            let name_max_width = title_width.saturating_sub(prefix_width).max(1);
            let display_name = truncate_display_width(&name, name_max_width);

            let mut spans = Vec::new();
            spans.push(Span::raw(" "));
            if !icon_glyph.is_empty() {
                spans.push(Span::styled(icon_glyph, icon_style));
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(
                display_name.clone(),
                Style::default().fg(active_theme.text_normal),
            ));

            let used_width = prefix_width + UnicodeWidthStr::width(display_name.as_str());
            if used_width < title_width {
                spans.push(Span::raw(" "));
            }
            Line::from(spans)
        } else {
            Line::from(Span::raw(" Preview "))
        };

        let preview_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(preview_title)
            .border_style(Style::default().fg(if app.preview_focus_is_preview() {
                active_theme.text_normal
            } else {
                active_theme.border
            }))
            .style(Style::default().bg(active_theme.bg_panel).fg(active_theme.text_normal));
        let preview_inner = preview_block.inner(preview_area);
        f.render_widget(preview_block, preview_area);

        let preview_chunks = if app.preview.footer.is_some() && preview_inner.height > 1 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(preview_inner)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(0)])
                .split(preview_inner)
        };
        let preview_body = preview_chunks[0];
        let preview_footer_area = preview_chunks[1];

        let preview_needs_scroll = app.preview.lines.len() > preview_body.height as usize;
        let preview_can_draw_scrollbar = preview_body.width > 2 && preview_needs_scroll;
        let preview_text_area = if preview_can_draw_scrollbar {
            Rect::new(
                preview_body.x,
                preview_body.y,
                preview_body.width.saturating_sub(1),
                preview_body.height,
            )
        } else {
            preview_body
        };

        app.preview.native_area = Some(preview_text_area);

        let visible_rows = preview_text_area.height.max(1) as usize;
        let max_scroll = app.preview.lines.len().saturating_sub(visible_rows);
        let offset = app.preview.scroll_offset.min(max_scroll);
        app.preview.scroll_offset = offset;

        let preview_protocol = App::terminal_image_protocol().0;
        let native_pane_image = app.is_preview_mode()
            && matches!(
                preview_protocol,
                crate::integration::probe::TerminalImageProtocol::Kitty
                    | crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                    | crate::integration::probe::TerminalImageProtocol::Sixel
            )
            && app.preview.image_rgb.is_some();

        let rendered_lines: Vec<Line> = if let Some((ref rgb, iw, ih)) = app.preview.image_rgb {
            if native_pane_image {
                vec![Line::from(Span::raw(" ".repeat(preview_text_area.width as usize))); preview_text_area.height as usize]
            } else {
                App::halfblock_lines(rgb, iw, ih, preview_text_area.width, preview_text_area.height)
            }
        } else {
            let is_directory_preview = app
                .preview.target_path
                .as_ref()
                .map(|path| path.is_dir())
                .unwrap_or(false);
            let mut tlines: Vec<Line> = app
                .preview.lines
                .iter()
                .skip(offset)
                .take(visible_rows)
                .enumerate()
                .map(|(idx, line)| {
                    if is_directory_preview {
                        ui::preview::render_directory_preview_line(
                            line,
                            app.preview.line_kinds.get(offset + idx).copied(),
                        )
                    } else {
                        let spans = ui::ansi::parse_ansi_line(line);
                        Line::from(spans)
                    }
                })
                .collect();
            if tlines.is_empty() {
                tlines.push(Line::from(Span::styled(
                    "No preview",
                    Style::default().fg(active_theme.text_dim),
                )));
            }
            tlines
        };
        f.render_widget(Paragraph::new(rendered_lines), preview_text_area);

        if let Some(footer_text) = app.preview.footer.as_ref()
            && preview_footer_area.height > 0 {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        footer_text.as_str(),
                        Style::default().fg(active_theme.divider),
                    )))
                    .alignment(Alignment::Right),
                    preview_footer_area,
                );
            }

        if preview_can_draw_scrollbar {
            let sb_area = Rect::new(
                preview_area.x + preview_area.width.saturating_sub(1),
                preview_body.y,
                1,
                preview_body.height,
            );
            ui::scrollbar::render_scrollbar_track(
                f,
                sb_area,
                app.preview.lines.len(),
                visible_rows,
                offset,
                active_theme.divider,
                active_theme.border,
            );
        }
        } else if app.is_dual_panel_mode() {
            let right_path: &std::path::PathBuf = if app.right.dir.as_os_str().is_empty() {
                &app.left.dir
            } else {
                &app.right.dir
            };
            let right_title = build_panel_title(
                right_path,
                app.display_path_for(right_path),
                false,
                preview_area.width,
                &title_style(app, active_theme),
            );

            let right_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(right_title)
                .border_style(Style::default().fg(if app.active_panel == crate::DualPanelSide::Right {
                    active_theme.text_normal
                } else {
                    active_theme.border
                }))
                .style(Style::default().bg(active_theme.bg_panel).fg(active_theme.text_normal));
            f.render_widget(right_block, preview_area);
            let right_body_area = Rect::new(
                preview_area.x + 1,
                preview_area.y + 1,
                preview_area.width.saturating_sub(2),
                preview_area.height.saturating_sub(2),
            );
            let right_body_area = if app.folder_filter_on_right() {
                render_folder_filter_box(
                    f,
                    app,
                    right_body_area,
                    active_theme,
                    app.mode == AppMode::FolderFilter,
                )
            } else {
                right_body_area
            };

            let right_term_w = right_body_area.width.max(1);
            let right_show_date = right_term_w >= 50;
            let right_show_size = right_term_w >= 70;
            let right_show_pct = app.size.folder_size_enabled && right_show_size;
            let right_size_min_max = if right_show_size { app.right.list_aggregates.size_min_max } else { None };
            let right_date_rank_by_ts = &app.right.list_aggregates.date_rank_by_ts;
            let right_size_width = if right_show_size { app.right.list_aggregates.max_size_width } else { 1 };
            let right_pct_width = 4usize;
            let right_date_width = 16usize;
            let right_total_for_pct = if right_show_pct { app.right.list_aggregates.percent_total } else { None };
            let right_name_width = panel_name_width(
                right_term_w,
                right_show_size,
                right_size_width,
                right_show_pct,
                right_pct_width,
                right_show_date,
                right_date_width,
            );

            let right_pill_edge_width = 2usize;
            let right_effective_name_width = right_name_width.saturating_sub(right_pill_edge_width).max(1);
            let right_table_render_area = Rect::new(
                right_body_area.x,
                right_body_area.y,
                right_body_area.width.saturating_sub(1),
                right_body_area.height,
            );
            let right_pill_color = if app.active_panel == crate::DualPanelSide::Right {
                active_theme.bg_selected
            } else {
                active_theme.bg_inactive_panel
            };

            let right_rows: Vec<Row> = app
                .right.entry_render_cache
                .iter()
                .enumerate()
                .map(|(idx, entry_cache)| {
                    let right_is_marked = app.right.marked_indices.contains(&idx);
                    let right_is_selected = idx == app.right.selected_index;
                    // Dark foreground for selected rows on a light selection bg.
                    let right_sel_override = if right_is_selected && ui::palette::is_light_bg(right_pill_color) {
                        Some(Color::Black)
                    } else {
                        None
                    };
                    let right_apply_sel = |style: Style| match right_sel_override {
                        Some(fg) => style.fg(fg),
                        None => style,
                    };
                    let tree_prefix = app
                        .right.tree_row_prefixes
                        .get(idx)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let icon_prefix_width = if app.show_icons && !entry_cache.icon_glyph.is_empty() {
                        2usize
                    } else {
                        0usize
                    };
                    let prefix_width = tree_prefix.chars().count();
                    let available_name_width = right_effective_name_width
                        .saturating_sub(prefix_width + icon_prefix_width)
                        .max(1);
                    let right_note_text = app
                        .notes.right_by_name
                        .get(&entry_cache.raw_name)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let name = truncate_with_ellipsis(&entry_cache.raw_name, available_name_width);
                    let right_row_fill = right_apply_sel(Style::default().bg(right_pill_color));
                    let mut spans = Vec::new();
                    if right_is_selected {
                        if app.nerd_font_active {
                            spans.push(Span::styled(
                                "",
                                Style::default().fg(right_pill_color).bg(active_theme.bg_panel),
                            ));
                        } else {
                            spans.push(Span::styled(" ", Style::default().bg(right_pill_color)));
                        }
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    if !tree_prefix.is_empty() {
                        let style = if right_is_selected { tree_style.patch(right_row_fill) } else { tree_style };
                        spans.push(Span::styled(tree_prefix.to_string(), style));
                    }
                    if app.show_icons && !entry_cache.icon_glyph.is_empty() {
                        let style = if right_is_selected {
                            entry_cache.icon_style.patch(right_row_fill)
                        } else {
                            entry_cache.icon_style
                        };
                        spans.push(Span::styled(format!("{} ", entry_cache.icon_glyph), style));
                    }
                    let name_style = if right_is_selected {
                        entry_cache.name_style.patch(right_row_fill)
                    } else {
                        entry_cache.name_style
                    };
                    let mut rendered_right_note = String::new();
                    if !right_note_text.is_empty() {
                        let used = prefix_width + icon_prefix_width + name.chars().count();
                        let sep = "  ";
                        let sep_len = sep.chars().count();
                        if used + sep_len < right_effective_name_width {
                            let remaining = right_effective_name_width - used - sep_len;
                            let clipped = truncate_with_ellipsis(right_note_text, remaining);
                            if !clipped.is_empty() {
                                rendered_right_note = format!("{}{}", sep, clipped);
                            }
                        }
                    }
                    spans.push(Span::styled(name, name_style));
                    if !rendered_right_note.is_empty() {
                        let right_note_style = if right_is_selected {
                            note_style.patch(right_row_fill)
                        } else {
                            note_style
                        };
                        spans.push(Span::styled(rendered_right_note, right_note_style));
                    }
                    let used_inner: usize = spans
                        .iter()
                        .skip(1)
                        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                        .sum();
                    if right_effective_name_width > used_inner {
                        spans.push(Span::styled(
                            " ".repeat(right_effective_name_width - used_inner),
                            if right_is_selected { right_row_fill } else { Style::default() },
                        ));
                    }
                    if right_is_selected {
                        if app.nerd_font_active {
                            spans.push(Span::styled(
                                "",
                                Style::default().fg(right_pill_color).bg(active_theme.bg_panel),
                            ));
                        } else {
                            spans.push(Span::styled(" ", Style::default().bg(right_pill_color)));
                        }
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    let name_cell = Cell::from(Line::from(spans));
                    let mut cells = vec![name_cell];
                    let right_size_style = right_apply_sel(Style::default().fg(ui::list_temperature::size_color_for(
                        entry_cache.size_bytes,
                        right_size_min_max,
                    )));
                    let right_date_style = right_apply_sel(Style::default().fg(ui::list_temperature::date_color_for(
                        entry_cache.modified_unix,
                        right_date_rank_by_ts,
                    )));
                    push_metric_cells(
                        &mut cells,
                        entry_cache,
                        &MetricColumns {
                            size: right_show_size.then_some((right_size_width, right_size_style)),
                            pct: right_show_pct.then_some((right_pct_width, right_total_for_pct)),
                            date: right_show_date.then_some(right_date_style),
                        },
                    );
                    Row::new(cells).style(if right_is_selected {
                        right_selection_style
                    } else if right_is_marked {
                        Style::default().bg(active_theme.search_match_bg)
                    } else {
                        Style::default()
                    })
                })
                .collect();

            let mut right_constraints: Vec<Constraint> = vec![Constraint::Min(0)];
            push_metric_constraints(
                &mut right_constraints,
                right_show_size,
                right_size_width,
                right_show_pct,
                right_pct_width,
                right_show_date,
                right_date_width,
            );
            let right_table = Table::new(right_rows, right_constraints)
                .highlight_style(Style::default())
                .highlight_symbol("");
            app.right.table_state.select(Some(app.right.selected_index));
            f.render_stateful_widget(right_table, right_table_render_area, &mut app.right.table_state);

            if let Some(sel) = app.right.table_state.selected() {
                let offset = app.right.table_state.offset();
                if sel >= offset {
                    let row_in_view = sel - offset;
                    if row_in_view < right_table_render_area.height as usize {
                        let cap_area = Rect::new(
                            right_table_render_area.x + right_table_render_area.width,
                            right_table_render_area.y + row_in_view as u16,
                            1,
                            1,
                        );
                        let cap_span = if app.nerd_font_active {
                            Span::styled(
                                "",
                                Style::default()
                                    .fg(right_pill_color)
                                    .bg(active_theme.bg_panel),
                            )
                        } else {
                            Span::styled(" ", Style::default().bg(right_pill_color))
                        };
                        f.render_widget(Paragraph::new(cap_span), cap_area);
                    }
                }
            }

            // Render right panel scrollbar
            let right_needs_scroll = app.right.entries.len() > right_body_area.height as usize;
            let right_can_draw_scrollbar = right_body_area.width > 2 && right_needs_scroll;
            if right_can_draw_scrollbar {
                let right_sb_area = Rect::new(
                    preview_area.x + preview_area.width.saturating_sub(1),
                    right_body_area.y,
                    1,
                    right_body_area.height,
                );
                let right_visible_rows = right_body_area.height.max(1) as usize;
                ui::scrollbar::render_scrollbar_track(
                    f,
                    right_sb_area,
                    app.right.entries.len(),
                    right_visible_rows,
                    app.right.table_state.offset(),
                    active_theme.divider,
                    active_theme.border,
                );
            }
        }
    }

    if app.is_preview_mode() || app.is_dual_panel_mode() {
        let active_side = if app.is_dual_panel_mode() {
            app.active_panel
        } else {
            crate::DualPanelSide::Left
        };
        let active_status = if app.copy.rx.is_none() && app.archive.rx.is_none() {
            app.selected_total_size_status_for(active_side)
        } else {
            None
        }
        .or_else(|| app.panel_status_message(active_side).map(|s| s.to_string()));

        let use_preview_frame = (app.is_dual_panel_mode() && active_side == crate::DualPanelSide::Right)
            || (app.is_preview_mode() && app.preview_focus_is_preview());
        let active_frame_area = if use_preview_frame { preview_frame_area } else { Some(list_frame_area) };

        if let (Some(status_text), Some(frame_area)) = (active_status, active_frame_area) {
            let lower_msg = status_text.to_ascii_lowercase();
            let selected_total_is_shown = lower_msg.starts_with("selected:");
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
            let msg_area = Rect::new(
                frame_area.x.saturating_add(1),
                frame_area.y + frame_area.height.saturating_sub(1),
                frame_area.width.saturating_sub(2),
                1,
            );
            if msg_area.width > 0 {
                let decorated = app.decorate_footer_message(&status_text);
                let core = format!("─── {} ", decorated);
                let width = msg_area.width as usize;
                let core_len = core.chars().count();
                let line_msg = if core_len >= width {
                    core.chars().take(width).collect::<String>()
                } else {
                    format!("{}{}", core, "─".repeat(width - core_len))
                };
                f.render_widget(Paragraph::new(line_msg).style(msg_style), msg_area);
            }
        }
    }

}

