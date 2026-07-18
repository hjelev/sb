use super::*;
use crate::ui::list_metrics::*;

pub(crate) fn render_table(f: &mut Frame, app: &mut App, ctx: &RenderCtx) -> TableLayout {
    let active_theme = ctx.theme;
    let chunks = [ctx.main, ctx.footer];
    let header_reserved_rows = ctx.header_rows;

    // --- Table ---
    let content_area = Rect::new(
        chunks[0].x,
        chunks[0].y + header_reserved_rows,
        chunks[0].width,
        chunks[0].height.saturating_sub(header_reserved_rows),
    );
    let (list_frame_area, preview_frame_area) = if app.is_preview_mode() {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Percentage(67)])
            .split(content_area);
        (split[0], Some(split[1]))
    } else if app.is_dual_panel_mode() {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_area);
        (split[0], Some(split[1]))
    } else {
        (content_area, None)
    };



    if app.is_preview_mode() || app.is_dual_panel_mode() {
        let path_text = if app.mode == AppMode::PathEditing {
            app.input_buffer.clone()
        } else {
            app.current_dir_display_path_with_filter()
        };
        let left_title = build_panel_title(
            &app.left.dir,
            path_text,
            app.mode == AppMode::PathEditing,
            list_frame_area.width,
            &title_style(app, active_theme),
        );

        let left_border_color = if app.is_dual_panel_mode() {
            if app.active_panel == crate::DualPanelSide::Left {
                active_theme.text_normal
            } else {
                active_theme.border
            }
        } else if app.preview_focus_is_preview() {
            active_theme.border
        } else {
            active_theme.text_normal
        };
        let left_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(left_title)
            .border_style(Style::default().fg(left_border_color));
        let left_block = left_block.style(
            Style::default()
                .bg(active_theme.bg_panel)
                .fg(active_theme.text_normal),
        );
        f.render_widget(left_block, list_frame_area);
    }

    let term_w = if app.is_preview_mode() || app.is_dual_panel_mode() {
        list_frame_area.width.saturating_sub(2)
    } else {
        chunks[0].width
    };
    let show_date = term_w >= 50;
    let show_size = term_w >= 70;
    let show_meta = !app.is_preview_mode() && !app.is_dual_panel_mode() && term_w >= 90;
    let show_pct = app.size.folder_size_enabled && show_size;
    let perms_width = 11usize;
    let group_width = app.meta.group_width.max(1);
    let owner_width = app.meta.owner_width.max(1);
    let size_width = if show_size { app.left.list_aggregates.max_size_width } else { 1 };
    let pct_width = 4usize;
    let date_width = 16usize;
    let reserved_width = (if show_meta { perms_width + group_width + owner_width } else { 0 })
        + (if show_size { size_width } else { 0 })
        + (if show_pct { pct_width } else { 0 })
        + (if show_date { date_width } else { 0 });
    let name_cell_width = (term_w as usize).saturating_sub(reserved_width);
    // Keep a small safety margin so truncation occurs before the table widget clips.
    let file_name_width = name_cell_width.saturating_sub(6).max(1);


    let note_style = Style::default().fg(active_theme.text_dim);
    let tree_style = Style::default().fg(active_theme.text_dim);

    // Keep selected-row background while preserving per-span foreground colors
    // (e.g. filename white, note text gray).
    let selection_style = if app.is_dual_panel_mode() {
        match app.active_panel {
            crate::DualPanelSide::Left => Style::default().bg(active_theme.bg_selected),
            crate::DualPanelSide::Right => Style::default().bg(active_theme.bg_inactive_panel),
        }
    } else {
        Style::default().bg(active_theme.bg_selected)
    };
    let marker_width = if app.no_color { 3 } else { 0 };
    let name_cell_text_width = name_cell_width.saturating_sub(marker_width).max(1);
    let name_truncate_width = file_name_width.saturating_sub(marker_width).max(1);
    let entry_styles = |mut icon_style: Style, mut name_style: Style, is_selected: bool| {
        if app.no_color && !is_selected {
            icon_style.fg = None;
            name_style.fg = None;
        }
        (icon_style, name_style)
    };

    // Aggregates are precomputed when the render cache is (re)built; the render
    // path only gates them by the column-visibility flags here.
    let size_min_max = if show_size { app.left.list_aggregates.size_min_max } else { None };
    let left_total_for_pct = if show_pct { app.left.list_aggregates.percent_total } else { None };
    let date_rank_by_ts = &app.left.list_aggregates.date_rank_by_ts;
    let use_main_pill = true;
    let left_pill_color = if app.is_dual_panel_mode() && app.active_panel == crate::DualPanelSide::Right {
        active_theme.bg_inactive_panel
    } else {
        active_theme.bg_selected
    };

    let rows: Vec<Row> = app.left.entry_render_cache.iter().enumerate().map(|(idx, entry_cache)| {
        let is_marked = app.left.marked_indices.contains(&idx);
        let is_selected = idx == app.left.selected_index;
        let pill_mode = use_main_pill;
        let pill_selected = is_selected && pill_mode;
        let (icon_style, name_style) = entry_styles(entry_cache.icon_style, entry_cache.name_style, is_selected);

        // On a selected row with a light selection background (e.g. cyberpunk
        // neon's yellow), force a dark foreground so the per-type cell colors
        // (white names, cyan dirs, temperature shades) stay readable. Dark
        // selection backgrounds keep the cells' own colors.
        let sel_fg_override = if pill_selected && ui::palette::is_light_bg(left_pill_color) {
            Some(Color::Black)
        } else {
            None
        };
        let apply_sel = |style: Style| match sel_fg_override {
            Some(fg) => style.fg(fg),
            None => style,
        };

        let group_style = apply_sel(Style::default().fg(active_theme.meta_group));
        let owner_style = apply_sel(Style::default().fg(active_theme.meta_owner));
        let size_style = apply_sel(Style::default().fg(ui::list_temperature::size_color_for(
            entry_cache.size_bytes,
            size_min_max,
        )));
        let date_style =
            apply_sel(Style::default().fg(ui::list_temperature::date_color_for(
                entry_cache.modified_unix,
                date_rank_by_ts,
            )));
        let marker = if app.no_color {
            format!(
                "{}{} ",
                if is_selected { '>' } else { ' ' },
                if is_marked { '*' } else { ' ' }
            )
        } else {
            String::new()
        };
        let note_text = app
            .notes.by_name
            .get(&entry_cache.raw_name)
            .map(|s| s.as_str())
            .unwrap_or("");
        let tree_prefix = app.left.tree_row_prefixes.get(idx).map(|s| s.as_str()).unwrap_or("");
        let icon_prefix_width = if app.show_icons && !entry_cache.icon_glyph.is_empty() {
            2usize
        } else {
            0usize
        };
        let pill_edge_width = if pill_mode { 2usize } else { 0usize };
        let effective_name_width = name_cell_text_width.saturating_sub(pill_edge_width).max(1);
        let prefix_width = tree_prefix.chars().count();
        let available_name_width = name_truncate_width
            .saturating_sub(prefix_width + icon_prefix_width)
            .max(1);
        let rendered_name = truncate_with_ellipsis(&entry_cache.raw_name, available_name_width);
        let mut rendered_note = String::new();
        if !note_text.is_empty() {
            let used = prefix_width + icon_prefix_width + rendered_name.chars().count();
            let sep = "  ";
            let sep_len = sep.chars().count();
            if used + sep_len < name_truncate_width {
                let remaining = name_truncate_width - used - sep_len;
                let clipped_note = truncate_with_ellipsis(note_text, remaining);
                if !clipped_note.is_empty() {
                    rendered_note = format!("{}{}", sep, clipped_note);
                }
            }
        }

        let mut cells = vec![Cell::from(Line::from({
            let mut spans = vec![];
            let row_fill = apply_sel(Style::default().bg(left_pill_color));
            if pill_mode {
                if pill_selected {
                    if app.nerd_font_active {
                        spans.push(Span::styled(
                            "",
                            Style::default()
                                .fg(left_pill_color)
                                .bg(active_theme.bg_panel),
                        ));
                    } else {
                        spans.push(Span::styled(" ", Style::default().bg(left_pill_color)));
                    }
                } else {
                    spans.push(Span::raw(" "));
                }
            }
            if !marker.is_empty() {
                if pill_selected {
                    spans.push(Span::styled(marker, row_fill));
                } else {
                    spans.push(Span::raw(marker));
                }
            }
            if !tree_prefix.is_empty() {
                let style = if pill_selected {
                    tree_style.patch(row_fill)
                } else {
                    tree_style
                };
                spans.push(Span::styled(tree_prefix.to_string(), style));
            }
            if app.show_icons {
                let icon_text = format!("{} ", entry_cache.icon_glyph);
                let style = if pill_selected {
                    icon_style.patch(row_fill)
                } else {
                    icon_style
                };
                spans.push(Span::styled(icon_text, style));
            }
            let name_style = if pill_selected {
                name_style.patch(row_fill)
            } else {
                name_style
            };
            spans.push(Span::styled(rendered_name, name_style));
            if !rendered_note.is_empty() {
                let note_style = if pill_selected {
                    note_style.patch(row_fill)
                } else {
                    note_style
                };
                spans.push(Span::styled(rendered_note, note_style));
            }
            if pill_mode {
                let used_inner: usize = spans
                    .iter()
                    .skip(1)
                    .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                    .sum();
                if effective_name_width > used_inner {
                    spans.push(Span::styled(
                        " ".repeat(effective_name_width - used_inner),
                        if pill_selected { row_fill } else { Style::default() },
                    ));
                }
                if pill_selected {
                    if app.nerd_font_active {
                        spans.push(Span::styled(
                            "",
                            Style::default()
                                .fg(left_pill_color)
                                .bg(active_theme.bg_panel),
                        ));
                    } else {
                        spans.push(Span::styled(" ", Style::default().bg(left_pill_color)));
                    }
                } else {
                    spans.push(Span::raw(" "));
                }
            }
            spans
        }))];
        if show_meta {
            let perms_text = entry_cache.perms_col.trim();
            let perms_spans: Vec<Span> = ui::list_render::permission_gradient_segments(
                perms_text,
                perms_width,
            )
            .into_iter()
            .map(|(text, color)| match sel_fg_override.or(color) {
                Some(c) => Span::styled(text, Style::default().fg(c)),
                None => Span::raw(text),
            })
            .collect();
            cells.push(Cell::from(Line::from(perms_spans)));
            cells.push(Cell::from(Span::styled(
                format!("{:>width$}", entry_cache.group_name, width = group_width),
                group_style,
            )));
            cells.push(Cell::from(Span::styled(
                format!("{:<width$}", entry_cache.owner_name, width = owner_width),
                owner_style,
            )));
        }
        push_metric_cells(
            &mut cells,
            entry_cache,
            &MetricColumns {
                size: show_size.then_some((size_width, size_style)),
                pct: show_pct.then_some((pct_width, left_total_for_pct)),
                date: show_date.then_some(date_style),
            },
        );
        Row::new(cells).style(if is_selected {
            Style::default().bg(left_pill_color)
        } else if is_marked {
            Style::default().bg(active_theme.search_match_bg)
        } else {
            Style::default()
        })
    }).collect();

    let mut col_constraints: Vec<Constraint> = vec![Constraint::Min(0)];
    if show_meta {
        col_constraints.push(Constraint::Length(perms_width as u16));
        col_constraints.push(Constraint::Length(group_width as u16));
        col_constraints.push(Constraint::Length(owner_width as u16));
    }
    push_metric_constraints(
        &mut col_constraints,
        show_size,
        size_width,
        show_pct,
        pct_width,
        show_date,
        date_width,
    );
    let table = Table::new(rows, col_constraints)
        .highlight_style(Style::default())
        .highlight_symbol("");

    let table_area = if app.is_preview_mode() || app.is_dual_panel_mode() {
        Rect::new(
            list_frame_area.x + 1,
            list_frame_area.y + 1,
            list_frame_area.width.saturating_sub(2),
            list_frame_area.height.saturating_sub(2),
        )
    } else {
        content_area
    };
    let table_area = if app.folder_filter_on_left() {
        render_folder_filter_box(
            f,
            app,
            table_area,
            active_theme,
            app.mode == AppMode::FolderFilter,
        )
    } else {
        table_area
    };
    let needs_scroll = app.left.entries.len() > table_area.height as usize;
    let can_draw_scrollbar = app.mode_shows_main_scrollbar() && table_area.width > 2 && needs_scroll;
    let list_area = if can_draw_scrollbar {
        Rect::new(table_area.x, table_area.y, table_area.width.saturating_sub(1), table_area.height)
    } else {
        table_area
    };
    let table_render_area = if use_main_pill {
        Rect::new(
            list_area.x,
            list_area.y,
            list_area.width.saturating_sub(1),
            list_area.height,
        )
    } else {
        list_area
    };
    app.page_size = (table_render_area.height as usize).saturating_sub(1).max(1);
    f.render_stateful_widget(table, table_render_area, &mut app.left.table_state);

    if app.left.entries.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No files or folders yet. Use the 'n' button to break the silence.",
                Style::default()
                        .fg(active_theme.text_dim)
                    .add_modifier(Modifier::ITALIC),
            )))
            .alignment(Alignment::Left),
            table_render_area,
        );
    }

    // If the selected item is truncated, temporarily hide its metadata and
    // render its full name across the whole row width.
    if let Some(selected_idx) = app.left.table_state.selected()
        && let Some(entry_cache) = app.left.entry_render_cache.get(selected_idx) {
            let tree_prefix = app.left.tree_row_prefixes.get(selected_idx).map(|s| s.as_str()).unwrap_or("");
            let full_name = entry_cache.raw_name.as_str();
            let prefix_width_for_check = tree_prefix.chars().count();
            if prefix_width_for_check + full_name.chars().count() > name_truncate_width {
                let offset = app.left.table_state.offset();
                if selected_idx >= offset {
                    let row_in_view = selected_idx - offset;
                    if row_in_view < table_render_area.height as usize {
                        let row_area = Rect::new(
                            table_render_area.x,
                            table_render_area.y + row_in_view as u16,
                            table_render_area.width,
                            1,
                        );
                        let is_marked = app.left.marked_indices.contains(&selected_idx);
                        let icon_style = entry_cache.icon_style.fg(active_theme.text_normal);
                        let name_style = entry_cache.name_style.fg(active_theme.text_normal);
                        let marker = if app.no_color {
                            format!(">{} ", if is_marked { '*' } else { ' ' })
                        } else {
                            String::new()
                        };
                        let note_text = app
                            .notes.by_name
                            .get(entry_cache.raw_name.as_str())
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        let note_suffix = if note_text.is_empty() {
                            String::new()
                        } else {
                            format!("  {}", note_text)
                        };

                        f.render_widget(Clear, row_area);
                        let pill_selected = use_main_pill;
                        f.render_widget(
                            Block::default().style(selection_style),
                            row_area,
                        );
                        f.render_widget(
                            Paragraph::new(Line::from({
                                let mut spans = vec![];
                                if pill_selected {
                                    if app.nerd_font_active {
                                        spans.push(Span::styled(
                                            "",
                                            Style::default()
                                                .fg(left_pill_color)
                                                .bg(active_theme.bg_panel),
                                        ));
                                    } else {
                                        spans.push(Span::styled(" ", Style::default().bg(left_pill_color)));
                                    }
                                }
                                if !marker.is_empty() {
                                    if pill_selected {
                                        spans.push(Span::styled(
                                            marker,
                                            Style::default().bg(left_pill_color),
                                        ));
                                    } else {
                                        spans.push(Span::raw(marker));
                                    }
                                }
                                if !tree_prefix.is_empty() {
                                    let style = if pill_selected {
                                        tree_style.bg(left_pill_color)
                                    } else {
                                        tree_style
                                    };
                                    spans.push(Span::styled(tree_prefix.to_string(), style));
                                }
                                if app.show_icons {
                                    let icon_text = format!("{} ", entry_cache.icon_glyph);
                                    let style = if pill_selected {
                                        icon_style.bg(left_pill_color)
                                    } else {
                                        icon_style
                                    };
                                    spans.push(Span::styled(icon_text, style));
                                }
                                let style = if pill_selected {
                                    name_style.bg(left_pill_color)
                                } else {
                                    name_style
                                };
                                spans.push(Span::styled(full_name.to_string(), style));
                                if !note_suffix.is_empty() {
                                    let style = if pill_selected {
                                        note_style.bg(left_pill_color)
                                    } else {
                                        note_style
                                    };
                                    spans.push(Span::styled(note_suffix, style));
                                }
                                if pill_selected {
                                    let used_inner: usize = spans
                                        .iter()
                                        .skip(1)
                                        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                                        .sum();
                                    let effective_name_width = name_cell_text_width.saturating_sub(2).max(1);
                                    if effective_name_width > used_inner {
                                        spans.push(Span::styled(
                                            " ".repeat(effective_name_width - used_inner),
                                            Style::default().bg(left_pill_color),
                                        ));
                                    }
                                    if app.nerd_font_active {
                                        spans.push(Span::styled(
                                            "",
                                            Style::default()
                                                .fg(left_pill_color)
                                                .bg(active_theme.bg_panel),
                                        ));
                                    } else {
                                        spans.push(Span::styled(" ", Style::default().bg(left_pill_color)));
                                    }
                                }
                                spans
                            })),
                            row_area,
                        );
                    }
                }
            }
        }

    if use_main_pill
        && let Some(selected_idx) = app.left.table_state.selected() {
            let offset = app.left.table_state.offset();
            if selected_idx >= offset {
                let row_in_view = selected_idx - offset;
                if row_in_view < table_render_area.height as usize {
                    let cap_area = Rect::new(
                        table_render_area.x + table_render_area.width,
                        table_render_area.y + row_in_view as u16,
                        1,
                        1,
                    );
                    let cap_span = if app.nerd_font_active {
                        Span::styled(
                            "",
                            Style::default()
                                .fg(left_pill_color)
                                .bg(active_theme.bg_panel),
                        )
                    } else {
                        Span::styled(" ", Style::default().bg(left_pill_color))
                    };
                    f.render_widget(Paragraph::new(cap_span), cap_area);
                }
            }
        }


    TableLayout {
        list_frame_area,
        preview_frame_area,
        table_area,
        can_draw_scrollbar,
        list_area,
    }
}

