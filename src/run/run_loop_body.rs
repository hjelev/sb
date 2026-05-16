use super::*;

pub(crate) fn run_tui_body(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> io::Result<()>
{
    let mut deferred_key: Option<KeyEvent> = None;
    let hostname = hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "host".to_string());
    let user = env::var("USER").unwrap_or_else(|_| "user".to_string());

    loop {
        app.refresh_header_clock_if_needed();
        app.pump_archive_progress();
        app.pump_copy_total_prescan();
        app.pump_copy_progress();
        app.pump_download_progress();
        app.pump_folder_size_progress();
        app.pump_recursive_mtime_progress();
        app.pump_current_dir_total_size_progress();
        app.pump_selected_total_size_progress();
        app.pump_git_info();
        app.pump_notes_progress();
        app.pump_internal_search_candidates_progress();
        app.pump_internal_search_content_progress();
        app.pump_preview_progress();
        app.request_preview_for_selected();
        let text_input_cursor = matches!(
            app.mode,
            AppMode::PathEditing
                | AppMode::DownloadInput
                | AppMode::DownloadNaming
                | AppMode::Renaming
                | AppMode::PasteRenaming
                | AppMode::NewFile
                | AppMode::NewFolder
                | AppMode::ArchiveCreate
                | AppMode::NoteEditing
                | AppMode::CommandInput
                | AppMode::GitCommitMessage
                | AppMode::GitTagInput
                | AppMode::InternalSearch
        );
        if text_input_cursor {
            execute!(terminal.backend_mut(), SetCursorStyle::BlinkingBar)?;
        } else {
            execute!(terminal.backend_mut(), SetCursorStyle::DefaultUserShape)?;
        }
        terminal.draw(|f| {
            let footer_height = 2;
            let header_reserved_rows = if app.is_preview_mode() || app.is_dual_panel_mode() { 1 } else { 2 };
            let chunks = Layout::default()
                .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
                .split(f.size());

            // Pre-calculate if scrollbar will be visible for header alignment
            let scrollbar_visible_in_main = {
                let table_area_height = chunks[0].height.saturating_sub(header_reserved_rows);
                let needs_scroll = app.entries.len() > table_area_height as usize;
                let table_area_width = if app.is_preview_mode() {
                    (chunks[0].width * 33 / 100).max(1)
                } else if app.is_dual_panel_mode() {
                    (chunks[0].width * 50 / 100).max(1)
                } else {
                    chunks[0].width
                };
                app.mode_shows_main_scrollbar() && table_area_width > 2 && needs_scroll
            };

            // --- Header ---
            let header_identity = app.current_header_identity(&user, &hostname);
            let current_display_path = if app.mode == AppMode::PathEditing {
                app.input_buffer.clone()
            } else {
                app.current_dir_display_path_with_filter()
            };
            let header_sep = if app.nerd_font_active { "\u{f0256} " } else { " » " };
            let os_icon_info: Option<(&'static str, Color)> = if app.nerd_font_active {
                // Use the remote OS icon if we're inside an SSH/rclone mount
                let active_remote_icon = app.ssh_mounts.iter()
                    .filter(|m| app.current_dir.starts_with(&m.mount_path))
                    .last()
                    .and_then(|m| m.remote_os_icon);
                active_remote_icon.or(app.os_icon)
            } else {
                None
            };
            let mut middle_spans: Vec<Span> = Vec::new();
            let os_icon_width: u16;
            if let (Some((glyph, color)), Some((left_identity, right_identity))) =
                (os_icon_info, header_identity.split_once('@'))
            {
                // Pad icon with a space on each side so the glyph has breathing room
                // and renders at a readable size across different terminals.
                let icon_text = format!("{} ", glyph);
                os_icon_width = UnicodeWidthStr::width(icon_text.as_str()) as u16;
                middle_spans.push(Span::raw(left_identity.to_string()));
                middle_spans.push(Span::styled(icon_text, Style::default().fg(color)));
                middle_spans.push(Span::raw(right_identity.to_string()));
            } else {
                // Fallback: prepend icon (with trailing space) then identity
                let os_icon_span: Option<Span> = os_icon_info.map(|(glyph, color)| {
                    Span::styled(format!("{} ", glyph), Style::default().fg(color))
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
                    middle_spans.push(Span::styled("@", Style::default().fg(Color::Rgb(120, 120, 120))));
                    middle_spans.push(Span::raw(right_identity.to_string()));
                } else {
                    middle_spans.push(Span::raw(header_identity.as_str()));
                }
            }

            let header_sep_span = if app.nerd_font_active {
                Span::styled(
                    header_sep,
                    Style::default()
                        .fg(Color::Rgb(100, 160, 240))
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
                        Span::styled(current_display_path.as_str(), Style::default().fg(Color::Rgb(255, 220, 120)))
                    } else {
                        Span::raw(current_display_path.as_str())
                    },
                ]
            };
            if app.integration_enabled("git") {
                if let Some((branch, is_dirty, tag_info)) = app.cached_git_info_for_current_dir() {
                    let branch_style = Style::default().fg(Color::Rgb(100, 150, 255));
                    left_spans.push(Span::styled(" (", branch_style));
                    left_spans.push(Span::styled(branch, branch_style));
                    if is_dirty {
                        left_spans.push(Span::styled("*", Style::default().fg(Color::White)));
                    }
                    if let Some((tag_name, ahead)) = tag_info {
                        let at_style = Style::default().fg(Color::Rgb(120, 120, 120));
                        let tag_style = Style::default().fg(Color::Rgb(80, 255, 120));
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
            }
            let header_right = if let Some(total_suffix) = app.current_dir_total_size_header_suffix() {
                let icon_style = Style::default().fg(Color::Rgb(100, 160, 240));
                let text_style = Style::default().fg(Color::White);
                let mut spans: Vec<Span> = Vec::new();
                let mut text_buf = String::new();
                for ch in total_suffix.chars() {
                    if ch == '\u{f10b7}' || ch == '\u{f02ca}' {
                        if !text_buf.is_empty() {
                            spans.push(Span::styled(text_buf.clone(), text_style));
                            text_buf.clear();
                        }
                        spans.push(Span::styled(ch.to_string(), icon_style));
                    } else {
                        text_buf.push(ch);
                    }
                }
                if !text_buf.is_empty() {
                    spans.push(Span::styled(text_buf, text_style));
                }
                Some(Line::from(spans))
            } else if !app.folder_size_enabled {
                Some(Line::from(vec![
                    Span::styled(app.header_clock_text.clone(), Style::default().fg(Color::White)),
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
                    Paragraph::new(Line::from(left_spans.clone())).alignment(Alignment::Left),
                    left_rect,
                );
            }
            if middle_rect.width > 0 {
                let middle_alignment = if show_right { Alignment::Center } else { Alignment::Right };
                f.render_widget(
                    Paragraph::new(Line::from(middle_spans.clone())).alignment(middle_alignment),
                    middle_rect,
                );
            }
            if show_right {
                if let Some(header_right_line) = header_right {
                    let scrollbar_offset = if scrollbar_visible_in_main { 1 } else { 0 };
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
                    .border_style(Style::default().fg(Color::DarkGray)),
                Rect::new(chunks[0].x, chunks[0].y + 1, chunks[0].width, 1),
            );

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
                let cwd_symlink = fs::symlink_metadata(&app.current_dir)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                let (folder_icon, folder_icon_style) = App::icon_for_path(
                    &app.current_dir,
                    app.show_icons,
                    app.nerd_font_active,
                    cwd_symlink,
                );
                let mut left_title_spans: Vec<Span> = Vec::new();
                left_title_spans.push(Span::raw(" "));
                if !folder_icon.is_empty() {
                    left_title_spans.push(Span::styled(folder_icon, folder_icon_style));
                    left_title_spans.push(Span::raw(" "));
                }
                if app.mode == AppMode::PathEditing {
                    left_title_spans.push(Span::styled(
                        path_text,
                        Style::default().fg(Color::Rgb(255, 220, 120)),
                    ));
                } else {
                    left_title_spans.push(Span::styled(path_text, Style::default().fg(Color::White)));
                }
                left_title_spans.push(Span::raw(" "));

                let left_border_color = if app.is_dual_panel_mode() {
                    if app.active_panel == crate::DualPanelSide::Left {
                        Color::White
                    } else {
                        Color::Rgb(120, 120, 120)
                    }
                } else if app.preview_focus_is_preview() {
                    Color::DarkGray
                } else {
                    Color::White
                };
                let left_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(Line::from(left_title_spans))
                    .border_style(Style::default().fg(left_border_color));
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
            let show_pct = app.folder_size_enabled && show_size;
            let perms_width = 11usize;
            let group_width = app.meta_group_width.max(1);
            let owner_width = app.meta_owner_width.max(1);
            let size_width = if show_size {
                app.entry_render_cache
                    .iter()
                    .map(|entry| entry.size_col.trim().chars().count())
                    .max()
                    .unwrap_or(1)
                    .max(1)
            } else {
                1
            };
            let pct_width = 4usize;
            let date_width = 16usize;
            let reserved_width = (if show_meta { perms_width + group_width + owner_width } else { 0 })
                + (if show_size { size_width } else { 0 })
                + (if show_pct { pct_width } else { 0 })
                + (if show_date { date_width } else { 0 });
            let name_cell_width = (term_w as usize).saturating_sub(reserved_width);
            // Keep a small safety margin so truncation occurs before the table widget clips.
            let file_name_width = name_cell_width.saturating_sub(6).max(1);

            let truncate_with_ellipsis = |s: &str, max: usize| -> String {
                if s.chars().count() <= max {
                    return s.to_string();
                }
                if max <= 1 {
                    return "…".to_string();
                }
                let mut out = String::new();
                for ch in s.chars().take(max - 1) {
                    out.push(ch);
                }
                out.push('…');
                out
            };

            let note_style = Style::default().fg(Color::Rgb(150, 150, 150));
            let tree_style = Style::default().fg(Color::Rgb(140, 140, 140));

            // Keep selected-row background while preserving per-span foreground colors
            // (e.g. filename white, note text gray).
            let (selection_style, right_selection_style) = if app.is_dual_panel_mode() {
                match app.active_panel {
                    crate::DualPanelSide::Left => (
                        Style::default().bg(Color::Rgb(40, 65, 110)),
                        Style::default().bg(Color::Rgb(38, 38, 45)),
                    ),
                    crate::DualPanelSide::Right => (
                        Style::default().bg(Color::Rgb(38, 38, 45)),
                        Style::default().bg(Color::Rgb(40, 65, 110)),
                    ),
                }
            } else {
                let s = Style::default().bg(Color::Rgb(50, 50, 50));
                (s, s)
            };
            let marker_width = if app.no_color { 3 } else { 0 };
            let name_text_width = file_name_width.saturating_sub(marker_width).max(1);
            let entry_styles = |mut icon_style: Style, mut name_style: Style, is_selected: bool| {
                if app.no_color && !is_selected {
                    icon_style.fg = None;
                    name_style.fg = None;
                }
                (icon_style, name_style)
            };

            let size_min_max = if show_size {
                ui::list_temperature::size_min_max_from_sizes(
                    app.entry_render_cache.iter().map(|entry| entry.size_bytes),
                )
            } else {
                None
            };

            let date_rank_by_ts = if show_date {
                ui::list_temperature::date_rank_map_from_unix(
                    app.entry_render_cache.iter().map(|entry| entry.modified_unix),
                )
            } else {
                HashMap::new()
            };

            let rows: Vec<Row> = app.entry_render_cache.iter().enumerate().map(|(idx, entry_cache)| {
                let is_marked = app.marked_indices.contains(&idx);
                let is_selected = idx == app.selected_index;
                let (icon_style, name_style) = entry_styles(entry_cache.icon_style, entry_cache.name_style, is_selected);

                let group_style = Style::default().fg(Color::Rgb(172, 136, 98));
                let owner_style = Style::default().fg(Color::Rgb(196, 172, 118));
                let size_style = Style::default().fg(ui::list_temperature::size_color_for(
                    entry_cache.size_bytes,
                    size_min_max,
                ));
                let pct_style = size_style;
                let date_style =
                    Style::default().fg(ui::list_temperature::date_color_for(
                        entry_cache.modified_unix,
                        &date_rank_by_ts,
                    ));
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
                    .notes_by_name
                    .get(&entry_cache.raw_name)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                let tree_prefix = app.tree_row_prefixes.get(idx).map(|s| s.as_str()).unwrap_or("");
                let icon_prefix_width = if app.show_icons && !entry_cache.icon_glyph.is_empty() {
                    if app.is_preview_mode() || app.is_dual_panel_mode() { 3usize } else { 2usize }
                } else {
                    0usize
                };
                let prefix_width = tree_prefix.chars().count();
                let available_name_width = name_text_width.saturating_sub(prefix_width + icon_prefix_width).max(1);
                let rendered_name = truncate_with_ellipsis(&entry_cache.raw_name, available_name_width);
                let mut rendered_note = String::new();
                if !note_text.is_empty() {
                    let used = prefix_width + icon_prefix_width + rendered_name.chars().count();
                    let sep = "  ";
                    let sep_len = sep.chars().count();
                    if used + sep_len < name_text_width {
                        let remaining = name_text_width - used - sep_len;
                        let clipped_note = truncate_with_ellipsis(note_text, remaining);
                        if !clipped_note.is_empty() {
                            rendered_note = format!("{}{}", sep, clipped_note);
                        }
                    }
                }

                let mut cells = vec![Cell::from(Line::from({
                    let mut spans = vec![];
                    if !marker.is_empty() {
                        spans.push(Span::raw(marker));
                    }
                    if !tree_prefix.is_empty() {
                        spans.push(Span::styled(tree_prefix.to_string(), tree_style));
                    }
                    if app.show_icons {
                        let icon_text = if app.is_preview_mode() || app.is_dual_panel_mode() {
                            format!(" {} ", entry_cache.icon_glyph)
                        } else {
                            format!("{} ", entry_cache.icon_glyph)
                        };
                        spans.push(Span::styled(icon_text, icon_style));
                    }
                    spans.push(Span::styled(rendered_name, name_style));
                    if !rendered_note.is_empty() {
                        spans.push(Span::styled(rendered_note, note_style));
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
                    .map(|(text, color)| match color {
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
                if show_size {
                    let size_col = format!("{:>width$}", entry_cache.size_col.trim(), width = size_width);
                    cells.push(Cell::from(Span::styled(size_col, size_style)));
                }
                if show_pct {
                    let pct_col = match (app.current_dir_total_size_bytes, entry_cache.size_bytes) {
                        (Some(total), Some(entry_bytes)) if total > 0 => {
                            let pct = (entry_bytes as f64 * 100.0) / (total as f64);
                            format!("{:>width$}", format!("{:.0}%", pct), width = pct_width)
                        }
                        _ => format!("{:>width$}", "-", width = pct_width),
                    };
                    cells.push(Cell::from(Span::styled(pct_col, pct_style)));
                }
                if show_date {
                    cells.push(Cell::from(Span::styled(entry_cache.date_col.as_str(), date_style)));
                }
                Row::new(cells).style(if is_marked { Style::default().bg(Color::Rgb(0, 100, 150)) } else { Style::default() })
            }).collect();

            let mut col_constraints: Vec<Constraint> = vec![Constraint::Min(0)];
            if show_meta {
                col_constraints.push(Constraint::Length(perms_width as u16));
                col_constraints.push(Constraint::Length(group_width as u16));
                col_constraints.push(Constraint::Length(owner_width as u16));
            }
            if show_size { col_constraints.push(Constraint::Length(size_width as u16)); }
            if show_pct { col_constraints.push(Constraint::Length(pct_width as u16)); }
            if show_date { col_constraints.push(Constraint::Length(date_width as u16)); }
            let table = Table::new(rows, col_constraints)
                .highlight_style(selection_style)
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
            let needs_scroll = app.entries.len() > table_area.height as usize;
            let can_draw_scrollbar = app.mode_shows_main_scrollbar() && table_area.width > 2 && needs_scroll;
            let list_area = if can_draw_scrollbar {
                Rect::new(table_area.x, table_area.y, table_area.width.saturating_sub(1), table_area.height)
            } else {
                table_area
            };
            app.page_size = (list_area.height as usize).saturating_sub(1).max(1);
            f.render_stateful_widget(table, list_area, &mut app.table_state);

            if app.entries.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "No files or folders yet. Use the 'n' button to break the silence.",
                        Style::default()
                            .fg(Color::Rgb(140, 140, 140))
                            .add_modifier(Modifier::ITALIC),
                    )))
                    .alignment(Alignment::Left),
                    list_area,
                );
            }

            // If the selected item is truncated, temporarily hide its metadata and
            // render its full name across the whole row width.
            if let Some(selected_idx) = app.table_state.selected() {
                if let Some(entry_cache) = app.entry_render_cache.get(selected_idx) {
                    let tree_prefix = app.tree_row_prefixes.get(selected_idx).map(|s| s.as_str()).unwrap_or("");
                    let full_name = entry_cache.raw_name.as_str();
                    if tree_prefix.chars().count() + full_name.chars().count() > file_name_width {
                        let offset = app.table_state.offset();
                        if selected_idx >= offset {
                            let row_in_view = selected_idx - offset;
                            if row_in_view < list_area.height as usize {
                                let row_area = Rect::new(
                                    list_area.x,
                                    list_area.y + row_in_view as u16,
                                    list_area.width,
                                    1,
                                );
                                let is_marked = app.marked_indices.contains(&selected_idx);
                                let icon_style = entry_cache.icon_style.fg(Color::White);
                                let name_style = entry_cache.name_style.fg(Color::White);
                                let marker = if app.no_color {
                                    format!(">{} ", if is_marked { '*' } else { ' ' })
                                } else {
                                    String::new()
                                };
                                let note_text = app
                                    .notes_by_name
                                    .get(entry_cache.raw_name.as_str())
                                    .map(|s| s.as_str())
                                    .unwrap_or("");
                                let note_suffix = if note_text.is_empty() {
                                    String::new()
                                } else {
                                    format!("  {}", note_text)
                                };

                                f.render_widget(Clear, row_area);
                                f.render_widget(
                                    Block::default().style(selection_style),
                                    row_area,
                                );
                                f.render_widget(
                                    Paragraph::new(Line::from({
                                        let mut spans = vec![];
                                        if !marker.is_empty() {
                                            spans.push(Span::raw(marker));
                                        }
                                        if !tree_prefix.is_empty() {
                                            spans.push(Span::styled(tree_prefix.to_string(), tree_style));
                                        }
                                        if app.show_icons {
                                            let icon_text = if app.is_preview_mode() || app.is_dual_panel_mode() {
                                                format!(" {} ", entry_cache.icon_glyph)
                                            } else {
                                                format!("{} ", entry_cache.icon_glyph)
                                            };
                                            spans.push(Span::styled(icon_text, icon_style));
                                        }
                                        spans.push(Span::styled(full_name.to_string(), name_style));
                                        if !note_suffix.is_empty() {
                                            spans.push(Span::styled(note_suffix, note_style));
                                        }
                                        spans
                                    })),
                                    row_area,
                                );
                            }
                        }
                    }
                }
            }

            // --- Bottom divider border ---
            let bottom_border_y = table_area.y + table_area.height;
            if !app.is_preview_mode() && !app.is_dual_panel_mode() && app.mode_shows_main_scrollbar() && bottom_border_y < chunks[0].y + chunks[0].height {
                f.render_widget(Block::default().borders(Borders::TOP).border_type(BorderType::Rounded).border_style(Style::default().fg(Color::DarkGray)), 
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
                let track_h = sb_area.height as usize;
                if track_h > 0 {
                    let visible_rows = list_area.height.max(1) as usize;
                    let total_rows = app.entries.len();
                    let max_scroll = total_rows.saturating_sub(visible_rows);
                    let offset = app.table_state.offset().min(max_scroll);
                    let thumb_h = ((visible_rows * track_h + total_rows.saturating_sub(1)) / total_rows)
                        .max(1)
                        .min(track_h);
                    let scroll_space = track_h.saturating_sub(thumb_h);
                    let thumb_y = if max_scroll == 0 {
                        0
                    } else {
                        (offset * scroll_space + (max_scroll / 2)) / max_scroll
                    };

                    let mut sb_lines: Vec<Line> = Vec::with_capacity(track_h);
                    for row in 0..track_h {
                        let in_thumb = row >= thumb_y && row < thumb_y + thumb_h;
                        let (ch, color) = if in_thumb {
                            ("┃", Color::Rgb(120, 200, 190))
                        } else {
                            ("│", Color::DarkGray)
                        };
                        sb_lines.push(Line::from(Span::styled(ch, Style::default().fg(color))));
                    }
                    f.render_widget(Paragraph::new(sb_lines), sb_area);
                }
            }

            app.preview_native_area = None;
            if let Some(preview_area) = preview_frame_area {
                if app.is_preview_mode() {
                let title_path = app
                    .preview_target_path
                    .clone()
                    .or_else(|| app.entries.get(app.selected_index).map(|e| e.path()));
                let preview_title = if let Some(path) = title_path {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .filter(|n| !n.is_empty())
                        .unwrap_or("Preview")
                        .to_string();
                    let is_symlink = fs::symlink_metadata(&path)
                        .map(|m| m.file_type().is_symlink())
                        .unwrap_or(false);
                    let (icon_glyph, icon_style) = App::icon_for_path(
                        &path,
                        app.show_icons,
                        app.nerd_font_active,
                        is_symlink,
                    );
                    let mut spans = Vec::new();
                    spans.push(Span::raw(" "));
                    if !icon_glyph.is_empty() {
                        spans.push(Span::styled(icon_glyph, icon_style));
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(name, Style::default().fg(Color::Rgb(220, 220, 220))));
                    spans.push(Span::raw(" "));
                    Line::from(spans)
                } else {
                    Line::from(Span::raw(" Preview "))
                };

                let preview_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(preview_title)
                    .border_style(Style::default().fg(if app.preview_focus_is_preview() {
                        Color::White
                    } else {
                        Color::DarkGray
                    }));
                let preview_inner = preview_block.inner(preview_area);
                f.render_widget(preview_block, preview_area);

                let preview_chunks = if app.preview_footer.is_some() && preview_inner.height > 1 {
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

                let preview_needs_scroll = app.preview_lines.len() > preview_body.height as usize;
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

                app.preview_native_area = Some(preview_text_area);

                let visible_rows = preview_text_area.height.max(1) as usize;
                let max_scroll = app.preview_lines.len().saturating_sub(visible_rows);
                let offset = app.preview_scroll_offset.min(max_scroll);
                app.preview_scroll_offset = offset;

                let preview_protocol = App::terminal_image_protocol().0;
                let native_pane_image = app.is_preview_mode()
                    && matches!(
                        preview_protocol,
                        crate::integration::probe::TerminalImageProtocol::Kitty
                            | crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                            | crate::integration::probe::TerminalImageProtocol::Sixel
                    )
                    && app.preview_image_rgb.is_some();

                let rendered_lines: Vec<Line> = if let Some((ref rgb, iw, ih)) = app.preview_image_rgb {
                    if native_pane_image {
                        vec![Line::from(Span::raw(" ".repeat(preview_text_area.width as usize))); preview_text_area.height as usize]
                    } else {
                        App::halfblock_lines(rgb, iw, ih, preview_text_area.width, preview_text_area.height)
                    }
                } else {
                    let is_directory_preview = app
                        .preview_target_path
                        .as_ref()
                        .map(|path| path.is_dir())
                        .unwrap_or(false);
                    let mut tlines: Vec<Line> = app
                        .preview_lines
                        .iter()
                        .skip(offset)
                        .take(visible_rows)
                        .enumerate()
                        .map(|(idx, line)| {
                            if is_directory_preview {
                                ui::preview::render_directory_preview_line(
                                    line,
                                    app.preview_line_kinds.get(offset + idx).copied(),
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
                            Style::default().fg(Color::Rgb(140, 140, 140)),
                        )));
                    }
                    tlines
                };
                f.render_widget(Paragraph::new(rendered_lines), preview_text_area);

                if let Some(footer_text) = app.preview_footer.as_ref() {
                    if preview_footer_area.height > 0 {
                        f.render_widget(
                            Paragraph::new(Line::from(Span::styled(
                                footer_text.clone(),
                                Style::default().fg(Color::Rgb(120, 200, 190)),
                            )))
                            .alignment(Alignment::Right),
                            preview_footer_area,
                        );
                    }
                }

                if preview_can_draw_scrollbar {
                    let sb_area = Rect::new(
                        preview_area.x + preview_area.width.saturating_sub(1),
                        preview_body.y,
                        1,
                        preview_body.height,
                    );
                    let track_h = sb_area.height as usize;
                    if track_h > 0 {
                        let thumb_h = ((visible_rows * track_h + app.preview_lines.len().saturating_sub(1))
                            / app.preview_lines.len())
                            .max(1)
                            .min(track_h);
                        let scroll_space = track_h.saturating_sub(thumb_h);
                        let thumb_y = if max_scroll == 0 {
                            0
                        } else {
                            (offset * scroll_space + (max_scroll / 2)) / max_scroll
                        };
                        let mut sb_lines: Vec<Line> = Vec::with_capacity(track_h);
                        for row in 0..track_h {
                            let in_thumb = row >= thumb_y && row < thumb_y + thumb_h;
                            let (ch, color) = if in_thumb {
                                ("┃", Color::Rgb(120, 200, 190))
                            } else {
                                ("│", Color::DarkGray)
                            };
                            sb_lines.push(Line::from(Span::styled(ch, Style::default().fg(color))));
                        }
                        f.render_widget(Paragraph::new(sb_lines), sb_area);
                    }
                }
                } else if app.is_dual_panel_mode() {
                    let right_path = if app.right_dir.as_os_str().is_empty() {
                        app.current_dir.clone()
                    } else {
                        app.right_dir.clone()
                    };
                    let right_title = {
                        let name = right_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .filter(|n| !n.is_empty())
                            .unwrap_or("Right")
                            .to_string();
                        let is_symlink = fs::symlink_metadata(&right_path)
                            .map(|m| m.file_type().is_symlink())
                            .unwrap_or(false);
                        let (icon_glyph, icon_style) = App::icon_for_path(
                            &right_path,
                            app.show_icons,
                            app.nerd_font_active,
                            is_symlink,
                        );
                        let mut spans = vec![Span::raw(" ")];
                        if !icon_glyph.is_empty() {
                            spans.push(Span::styled(icon_glyph, icon_style));
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(name, Style::default().fg(Color::Rgb(220, 220, 220))));
                        spans.push(Span::raw(" "));
                        Line::from(spans)
                    };

                    let right_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(right_title)
                        .border_style(Style::default().fg(if app.active_panel == crate::DualPanelSide::Right {
                            Color::White
                        } else {
                            Color::Rgb(120, 120, 120)
                        }));
                    let right_inner = right_block.inner(preview_area);
                    f.render_widget(right_block, preview_area);

                    let right_term_w = right_inner.width.saturating_sub(2).max(1);
                    let right_show_date = right_term_w >= 50;
                    let right_show_size = right_term_w >= 70;
                    let right_size_min_max = if right_show_size {
                        ui::list_temperature::size_min_max_from_sizes(
                            app.right_entry_render_cache.iter().map(|entry| entry.size_bytes),
                        )
                    } else {
                        None
                    };
                    let right_date_rank_by_ts = if right_show_date {
                        ui::list_temperature::date_rank_map_from_unix(
                            app.right_entry_render_cache.iter().map(|entry| entry.modified_unix),
                        )
                    } else {
                        HashMap::new()
                    };
                    let right_size_width = if right_show_size {
                        app.right_entry_render_cache
                            .iter()
                            .map(|entry| entry.size_col.trim().chars().count())
                            .max()
                            .unwrap_or(1)
                            .max(1)
                    } else {
                        1
                    };
                    let right_date_width = 16usize;
                    let right_name_width = (right_term_w as usize)
                        .saturating_sub((if right_show_size { right_size_width } else { 0 }) + if right_show_date { right_date_width } else { 0 })
                        .max(1);

                    let right_rows: Vec<Row> = app
                        .right_entry_render_cache
                        .iter()
                        .enumerate()
                        .map(|(idx, entry_cache)| {
                            let name = truncate_with_ellipsis(&entry_cache.raw_name, right_name_width);
                            let mut spans = Vec::new();
                            if app.show_icons && !entry_cache.icon_glyph.is_empty() {
                                spans.push(Span::raw(" "));
                                spans.push(Span::styled(format!("{} ", entry_cache.icon_glyph), entry_cache.icon_style));
                            }
                            spans.push(Span::styled(name, entry_cache.name_style));
                            let name_cell = Cell::from(Line::from(spans));
                            let mut cells = vec![name_cell];
                            if right_show_size {
                                let right_size_style = Style::default().fg(ui::list_temperature::size_color_for(
                                    entry_cache.size_bytes,
                                    right_size_min_max,
                                ));
                                cells.push(Cell::from(Span::styled(
                                    format!("{:>width$}", entry_cache.size_col.trim(), width = right_size_width),
                                    right_size_style,
                                )));
                            }
                            if right_show_date {
                                let right_date_style = Style::default().fg(ui::list_temperature::date_color_for(
                                    entry_cache.modified_unix,
                                    &right_date_rank_by_ts,
                                ));
                                cells.push(Cell::from(Span::styled(
                                    entry_cache.date_col.clone(),
                                    right_date_style,
                                )));
                            }
                            Row::new(cells).style(if idx == app.right_selected_index {
                                right_selection_style
                            } else {
                                Style::default()
                            })
                        })
                        .collect();

                    let mut right_constraints: Vec<Constraint> = vec![Constraint::Min(0)];
                    if right_show_size {
                        right_constraints.push(Constraint::Length(right_size_width as u16));
                    }
                    if right_show_date {
                        right_constraints.push(Constraint::Length(right_date_width as u16));
                    }
                    let right_table = Table::new(right_rows, right_constraints)
                        .highlight_style(right_selection_style)
                        .highlight_symbol("");
                    app.right_table_state.select(Some(app.right_selected_index));
                    f.render_stateful_widget(right_table, right_inner, &mut app.right_table_state);
                }
            }

            // --- Overlays ---
            let tab_overlay_anchor = {
                let area = chunks[0];
                let anchor_w = (area.width * 5 / 6).max(50).min(area.width);
                let anchor_h = (area.height * 5 / 6).max(12).min(area.height);
                Rect::new(
                    area.x + (area.width.saturating_sub(anchor_w)) / 2,
                    area.y + (area.height.saturating_sub(anchor_h)) / 2,
                    anchor_w,
                    anchor_h,
                )
            };
            if app.mode == AppMode::InternalSearch {
                let popup_area = Rect::new(
                    tab_overlay_anchor.x,
                    tab_overlay_anchor.y,
                    tab_overlay_anchor.width,
                    tab_overlay_anchor.height,
                );

                f.render_widget(Clear, popup_area);
                let popup_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(App::panel_tab_bar_line(app.panel_tab))
                    .title_style(Style::default().fg(Color::White))
                    .border_style(Style::default().fg(Color::Rgb(80, 200, 180)));
                let popup_inner = popup_block.inner(popup_area);
                f.render_widget(popup_block, popup_area);
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "x",
                        Style::default().fg(Color::Rgb(170, 170, 170)),
                    )),
                    App::tabbed_overlay_close_area(popup_area),
                );

                let search_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(1),
                        Constraint::Length(2),
                    ])
                    .split(popup_inner);
                let query_box_area = search_layout[0];
                let body_area = search_layout[1];
                let footer_area = search_layout[2];

                let query_box_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(95, 95, 95)));
                let query_inner = query_box_block.inner(query_box_area);
                f.render_widget(query_box_block, query_box_area);

                let (mode_text, mode_style) = if app.internal_search_scope == InternalSearchScope::Content {
                    (
                        "Scope: Content".to_string(),
                        Style::default().fg(Color::Rgb(120, 220, 180)),
                    )
                } else {
                    (
                        "Scope: Filename".to_string(),
                        Style::default().fg(Color::Rgb(120, 170, 255)),
                    )
                };
                let mode_width = UnicodeWidthStr::width(mode_text.as_str()) as u16;
                let query_row = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(1), Constraint::Length(mode_width + 1)])
                    .split(query_inner);
                let query_input_area = query_row[0];
                let query_mode_area = query_row[1];

                let query_icon = if app.show_icons && app.nerd_font_active { "\u{f002}" } else { "/" };
                let query_icon_prefix = format!(" {}  ", query_icon);
                let query_line = Line::from(vec![
                    Span::styled(query_icon_prefix.clone(), Style::default().fg(Color::Rgb(120, 180, 255))),
                    Span::styled(app.input_buffer.as_str(), Style::default().fg(Color::Rgb(255, 220, 120))),
                ]);
                f.render_widget(Paragraph::new(query_line), query_input_area);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(mode_text.clone(), mode_style))).alignment(Alignment::Right),
                    query_mode_area,
                );

                let mut lines: Vec<Line> = Vec::new();

                if app.internal_search_candidates_pending {
                    lines.push(Line::from(Span::styled(
                        "Indexing files asynchronously...",
                        Style::default().fg(Color::Rgb(120, 200, 255)),
                    )));
                } else if app.internal_search_candidates_truncated {
                    lines.push(Line::from(Span::styled(
                        "Indexed first 20000 files (refine query to narrow results)",
                        Style::default().fg(Color::Rgb(160, 160, 160)),
                    )));
                }

                if app.internal_search_scope == InternalSearchScope::Content {
                    let limits = app.internal_search_content_limits;
                    lines.push(Line::from(Span::styled(
                        format!(
                            " Limits: files={}  hits={}  max-file={}",
                            limits.max_files,
                            limits.max_hits,
                            App::format_size(limits.max_file_bytes as u64)
                        ),
                        Style::default().fg(Color::Rgb(160, 160, 160)),
                    )));

                    if app.internal_search_limits_menu_open {
                        let selected_style = Style::default().fg(Color::Rgb(255, 220, 120)).add_modifier(Modifier::BOLD);
                        let normal_style = Style::default().fg(Color::Rgb(180, 180, 180));
                        let item_line = |idx: usize, label: &str, value: String| {
                            let marker = if idx == app.internal_search_limits_selected { ">" } else { " " };
                            let style = if idx == app.internal_search_limits_selected {
                                selected_style
                            } else {
                                normal_style
                            };
                            Line::from(Span::styled(format!("{} {}: {}", marker, label, value), style))
                        };
                        lines.push(item_line(0, "Max files", limits.max_files.to_string()));
                        lines.push(item_line(1, "Max hits", limits.max_hits.to_string()));
                        lines.push(item_line(2, "Max file size", App::format_size(limits.max_file_bytes as u64)));
                        lines.push(Line::from(Span::styled(
                            "Editor: Up/Down select  Left/Right or +/- adjust  Shift=10x  r reset  Ctrl+L close",
                            Style::default().fg(Color::DarkGray),
                        )));
                    } else {
                        lines.push(Line::from(Span::styled(
                            " Ctrl+L open limits editor",
                            Style::default().fg(Color::DarkGray),
                        )));
                    }

                    if app.internal_search_content_pending {
                        lines.push(Line::from(Span::styled(
                            " Scanning content asynchronously...",
                            Style::default().fg(Color::Rgb(120, 200, 255)),
                        )));
                    }
                    if let Some(note) = &app.internal_search_content_limit_note {
                        lines.push(Line::from(Span::styled(
                            note.clone(),
                            Style::default().fg(Color::Rgb(160, 160, 160)),
                        )));
                    }
                }

                let selected = app.internal_search_selected;
                let body_content_w = body_area.width as usize;
                let visible_rows = body_area.height as usize;
                let header_rows = lines.len();
                let max_rows = visible_rows.saturating_sub(header_rows).max(1);
                let offset = if selected >= max_rows {
                    selected + 1 - max_rows
                } else {
                    0
                };
                let search_total_rows = app.internal_search_results.len();
                let search_max_scroll = search_total_rows.saturating_sub(max_rows);
                let search_scroll_offset = offset.min(search_max_scroll);
                let can_draw_search_scrollbar = body_area.width > 2 && search_total_rows > max_rows;

                if let Some(err) = &app.internal_search_regex_error {
                    lines.push(Line::from(Span::styled(
                        format!("Regex error: {}", err),
                        Style::default().fg(Color::Rgb(255, 120, 120)),
                    )));
                }

                if app.internal_search_results.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        " No matches",
                        Style::default().fg(Color::Rgb(180, 90, 90)),
                    )));
                } else {
                    for (display_idx, result_idx) in app
                        .internal_search_results
                        .iter()
                        .skip(offset)
                        .take(max_rows)
                        .enumerate()
                    {
                        let absolute_idx = offset + display_idx;
                        let is_selected = absolute_idx == selected;
                        let base_style = if is_selected {
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Rgb(60, 60, 60))
                        } else {
                            Style::default().fg(Color::Rgb(200, 200, 200))
                        };
                        let match_style = if is_selected {
                            Style::default()
                                .fg(Color::Rgb(255, 240, 170))
                                .bg(Color::Rgb(60, 60, 60))
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Rgb(255, 220, 120))
                                .add_modifier(Modifier::BOLD)
                        };
                        let marker = "  ";
                        let mut spans: Vec<Span> = vec![Span::styled(marker, base_style)];

                        let rel_path_for_icon = match result_idx {
                            InternalSearchResult::Filename { rel_path, .. } => rel_path,
                            InternalSearchResult::Content { rel_path, .. } => rel_path,
                        };
                        let abs_path = app.current_dir.join(rel_path_for_icon);
                        let is_symlink = abs_path
                            .symlink_metadata()
                            .map(|m| m.file_type().is_symlink())
                            .unwrap_or(false);
                        let is_dir = abs_path.is_dir();
                        let icon_name = rel_path_for_icon
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| rel_path_for_icon.to_string_lossy().into_owned());
                        let (icon_glyph, icon_style) = App::icon_for_name(
                            icon_name.as_str(),
                            is_dir,
                            app.show_icons,
                            app.nerd_font_active,
                            is_symlink,
                        );
                        let icon_span = if app.show_icons && !icon_glyph.is_empty() {
                            let adjusted_icon_style = if is_selected {
                                icon_style.bg(Color::Rgb(60, 60, 60))
                            } else {
                                icon_style
                            };
                            Some(Span::styled(format!("{} ", icon_glyph), adjusted_icon_style))
                        } else {
                            None
                        };

                        match result_idx {
                            InternalSearchResult::Filename { rel_path, match_ranges } => {
                                let rel_str = rel_path.to_string_lossy().into_owned();
                                let basename_start = rel_str.rfind('/').map(|idx| idx + 1).unwrap_or(0);
                                let (dir_part, base_part) = rel_str.split_at(basename_start);

                                let project_ranges = |start: usize, end: usize| -> Vec<(usize, usize)> {
                                    match_ranges
                                        .iter()
                                        .filter_map(|(rs, re)| {
                                            let overlap_start = (*rs).max(start);
                                            let overlap_end = (*re).min(end);
                                            if overlap_start < overlap_end {
                                                Some((overlap_start - start, overlap_end - start))
                                            } else {
                                                None
                                            }
                                        })
                                        .collect()
                                };

                                if !dir_part.is_empty() {
                                    let dir_ranges = project_ranges(0, basename_start);
                                    spans.extend(App::search_spans_with_ranges(
                                        dir_part,
                                        &dir_ranges,
                                        base_style,
                                        match_style,
                                    ));
                                }

                                if let Some(icon) = icon_span.clone() {
                                    spans.push(icon);
                                }

                                let base_ranges = project_ranges(basename_start, rel_str.len());
                                spans.extend(App::search_spans_with_ranges(
                                    base_part,
                                    &base_ranges,
                                    base_style,
                                    match_style,
                                ));
                            }
                            InternalSearchResult::Content {
                                rel_path,
                                line_number,
                                line_text,
                                match_ranges,
                            } => {
                                let path_text = rel_path.display().to_string();
                                let basename_start = path_text.rfind('/').map(|idx| idx + 1).unwrap_or(0);
                                let (dir_part, base_part) = path_text.split_at(basename_start);
                                if !dir_part.is_empty() {
                                    spans.push(Span::styled(
                                        dir_part.to_string(),
                                        base_style.fg(Color::Rgb(150, 190, 255)),
                                    ));
                                }
                                if let Some(icon) = icon_span {
                                    spans.push(icon);
                                }
                                spans.push(Span::styled(
                                    format!("{}:{}: ", base_part, line_number),
                                    base_style.fg(Color::Rgb(150, 190, 255)),
                                ));
                                spans.extend(App::search_spans_with_ranges(
                                    line_text,
                                    match_ranges,
                                    base_style,
                                    match_style,
                                ));
                            }
                        }

                        if is_selected {
                            let used_w: usize = spans
                                .iter()
                                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                                .sum();
                            if body_content_w > used_w {
                                spans.push(Span::styled(
                                    " ".repeat(body_content_w - used_w),
                                    base_style,
                                ));
                            }
                        }

                        lines.push(Line::from(spans));
                    }
                }

                f.render_widget(Paragraph::new(lines), body_area);
                if can_draw_search_scrollbar {
                    let sb_area = Rect::new(
                        popup_area.x + popup_area.width.saturating_sub(1),
                        body_area.y,
                        1,
                        body_area.height,
                    );
                    let track_h = sb_area.height as usize;
                    if track_h > 0 {
                        let thumb_h = ((max_rows * track_h + search_total_rows.saturating_sub(1)) / search_total_rows)
                            .max(1)
                            .min(track_h);
                        let scroll_space = track_h.saturating_sub(thumb_h);
                        let thumb_y = if search_max_scroll == 0 {
                            0
                        } else {
                            (search_scroll_offset * scroll_space + (search_max_scroll / 2)) / search_max_scroll
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
                }
                f.render_widget(
                    Paragraph::new(ui::panels::shortcut_footer_lines(&[
                        ("↑↓", "navigate"),
                        ("Enter", "open"),
                        ("Ctrl+T", "toggle scope"),
                        ("Regex", "re:pattern or /pattern/i"),
                        ("Tab", "switch tabs"),
                    ])),
                    footer_area,
                );

                app.clamp_input_cursor();
                let cursor_x = query_input_area.x
                    + UnicodeWidthStr::width(query_icon_prefix.as_str()) as u16
                    + app.input_cursor as u16;
                let cursor_y = query_input_area.y;
                f.set_cursor(
                    cursor_x.min(query_input_area.x + query_input_area.width.saturating_sub(1)),
                    cursor_y,
                );
            } else if app.mode == AppMode::DbPreview {
                let popup_area = Rect::new(
                    tab_overlay_anchor.x,
                    tab_overlay_anchor.y,
                    tab_overlay_anchor.width,
                    tab_overlay_anchor.height,
                );

                let db_title = app
                    .db_preview_path
                    .as_ref()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "SQLite Preview".to_string());

                let mut lines: Vec<Line> = vec![
                    Line::from(Span::styled(
                        "←→:switch table  Home/End:jump  Esc:close",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                let mut table_spans: Vec<Span> = vec![Span::styled(
                    "Tables: ",
                    Style::default().fg(Color::Rgb(160, 160, 160)),
                )];
                if app.db_preview_tables.is_empty() {
                    table_spans.push(Span::styled(
                        "(none)",
                        Style::default().fg(Color::Rgb(180, 90, 90)),
                    ));
                } else {
                    for (idx, table_name) in app.db_preview_tables.iter().enumerate() {
                        if idx > 0 {
                            table_spans.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
                        }
                        let display = if table_name.chars().count() > 20 {
                            let mut t = table_name.chars().take(19).collect::<String>();
                            t.push('…');
                            t
                        } else {
                            table_name.clone()
                        };
                        let style = if idx == app.db_preview_selected {
                            Style::default()
                                .fg(Color::Rgb(20, 20, 20))
                                .bg(Color::Rgb(120, 220, 140))
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Rgb(170, 210, 255))
                        };
                        table_spans.push(Span::styled(display, style));
                    }
                }
                lines.push(Line::from(table_spans));

                if let Some(err) = &app.db_preview_error {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        err.clone(),
                        Style::default().fg(Color::Rgb(255, 120, 120)),
                    )));
                } else {
                    lines.push(Line::from(""));
                    if app.db_preview_output_lines.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "(no rows)",
                            Style::default().fg(Color::Rgb(140, 140, 140)),
                        )));
                    } else {
                        let visible_w = popup_area.width.saturating_sub(4) as usize;
                        let clip_line = |text: &str| -> String {
                            if text.chars().count() <= visible_w {
                                return text.to_string();
                            }
                            if visible_w <= 1 {
                                return "…".to_string();
                            }
                            let mut out = text.chars().take(visible_w - 1).collect::<String>();
                            out.push('…');
                            out
                        };

                        for row in &app.db_preview_output_lines {
                            lines.push(Line::from(Span::styled(
                                clip_line(row),
                                Style::default().fg(Color::Rgb(210, 210, 210)),
                            )));
                        }
                    }
                }

                f.render_widget(Clear, popup_area);
                f.render_widget(
                    Paragraph::new(lines)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(format!(" SQLite: {} ", db_title))
                                .title_style(Style::default().fg(Color::White))
                                .border_style(Style::default().fg(Color::Rgb(120, 200, 150))),
                        )
                        .wrap(Wrap { trim: true }),
                    popup_area,
                );
            } else if app.mode == AppMode::Help {
                let (max_off, clamped_off) = ui::panels::render_help_overlay(
                    f,
                    tab_overlay_anchor,
                    app.panel_tab,
                    app.help_scroll_offset,
                );
                app.help_max_offset = max_off;
                app.help_scroll_offset = clamped_off;
            } else if matches!(app.mode, AppMode::NewFile | AppMode::NewFolder) {
                let area = f.size();
                let title = " Create ";
                let dialog_w = (area.width * 2 / 3).max(40).min(area.width.saturating_sub(4).max(1));

                let lines: Vec<&str> = if app.input_buffer.is_empty() {
                    vec![""]
                } else {
                    app.input_buffer.split('\n').collect()
                };
                let (cursor_line, cursor_col) = app.input_cursor_line_col();
                let max_content_lines = area.height.saturating_sub(7).max(1) as usize;
                let content_lines = lines.len().max(1).min(max_content_lines);
                let window_start = cursor_line.saturating_sub(content_lines.saturating_sub(1));
                let window_end = (window_start + content_lines).min(lines.len().max(1));
                let shown_lines = &lines[window_start..window_end];

                let dialog_h = (shown_lines.len() as u16 + 3).max(4).min(area.height.saturating_sub(2).max(1));
                let create_area = Rect::new(
                    (area.width.saturating_sub(dialog_w)) / 2,
                    (area.height.saturating_sub(dialog_h)) / 2,
                    dialog_w,
                    dialog_h,
                );

                f.render_widget(Clear, create_area);
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(title)
                    .title_style(Style::default().fg(Color::White))
                    .border_style(Style::default().fg(Color::Rgb(120, 120, 120)));
                let input_area = block.inner(create_area);
                f.render_widget(block, create_area);

                let create_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(1),
                        Constraint::Length(1),
                    ])
                    .split(input_area);
                let list_area = create_chunks[0];
                let help_area = create_chunks[1];

                let mut rendered_lines: Vec<Line> = Vec::new();
                for line in shown_lines {
                    let is_dir = if app.mode == AppMode::NewFolder {
                        true
                    } else {
                        line.trim_start().starts_with('/')
                    };
                    let icon_name = if is_dir {
                        line.trim_start().trim_start_matches('/').trim()
                    } else {
                        line.trim()
                    };
                    let (icon_glyph, icon_style) = App::icon_for_name(
                        icon_name,
                        is_dir,
                        app.show_icons,
                        app.nerd_font_active,
                        false,
                    );
                    let mut spans = Vec::new();
                    if app.show_icons && !icon_glyph.is_empty() {
                        spans.push(Span::styled(format!("{} ", icon_glyph), icon_style));
                    }
                    spans.push(Span::styled(*line, Style::default().fg(Color::Rgb(230, 230, 230))));
                    rendered_lines.push(Line::from(spans));
                }
                f.render_widget(Paragraph::new(rendered_lines), list_area);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "(/name = folder, name = file)  Alt+Enter: new line",
                        Style::default().fg(Color::DarkGray),
                    ))),
                    help_area,
                );

                let active_line_text = app.active_input_line_text();
                let active_is_dir = if app.mode == AppMode::NewFolder {
                    true
                } else {
                    active_line_text.trim_start().starts_with('/')
                };
                let active_icon_name = if active_is_dir {
                    active_line_text.trim_start().trim_start_matches('/').trim()
                } else {
                    active_line_text.trim()
                };
                let (active_icon_glyph, _) = App::icon_for_name(
                    active_icon_name,
                    active_is_dir,
                    app.show_icons,
                    app.nerd_font_active,
                    false,
                );
                let icon_prefix_width = if app.show_icons && !active_icon_glyph.is_empty() {
                    UnicodeWidthStr::width(format!("{} ", active_icon_glyph).as_str()) as u16
                } else {
                    0
                };

                app.clamp_input_cursor();
                let visible_cursor_line = cursor_line.saturating_sub(window_start);
                let cursor_x = list_area.x + icon_prefix_width + cursor_col as u16;
                let cursor_y = list_area.y + visible_cursor_line as u16;
                f.set_cursor(
                    cursor_x.min(list_area.x + list_area.width.saturating_sub(1)),
                    cursor_y.min(list_area.y + list_area.height.saturating_sub(1)),
                );
            } else if app.mode == AppMode::Renaming {
                let area = f.size();
                let selected_entry = app.entries.get(app.selected_index);
                let old_name = selected_entry
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .unwrap_or_else(|| app.input_buffer.clone());
                let selected_path = selected_entry.map(|e| e.path());
                let selected_is_dir = selected_path.as_ref().map(|p| p.is_dir()).unwrap_or(false);
                let selected_is_symlink = selected_path
                    .as_ref()
                    .and_then(|p| p.symlink_metadata().ok())
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                let dialog_w = (area.width * 2 / 3).max(36).min(area.width.saturating_sub(4).max(1));
                let dialog_h = 3u16.min(area.height.saturating_sub(2).max(1));
                let rename_area = Rect::new(
                    (area.width.saturating_sub(dialog_w)) / 2,
                    (area.height.saturating_sub(dialog_h)) / 2,
                    dialog_w,
                    dialog_h,
                );
                let title = format!(" Rename \"{}\" ", old_name);
                f.render_widget(Clear, rename_area);
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(title)
                    .title_style(Style::default().fg(Color::White))
                    .border_style(Style::default().fg(Color::Rgb(120, 120, 120)));
                let input_area = block.inner(rename_area);
                f.render_widget(block, rename_area);

                let (icon_glyph, icon_style) = App::icon_for_name(
                    app.input_buffer.as_str(),
                    selected_is_dir,
                    app.show_icons,
                    app.nerd_font_active,
                    selected_is_symlink,
                );
                let icon_prefix = if app.show_icons && !icon_glyph.is_empty() {
                    format!("{} ", icon_glyph)
                } else {
                    String::new()
                };
                let mut spans = Vec::new();
                if !icon_prefix.is_empty() {
                    spans.push(Span::styled(icon_prefix.clone(), icon_style));
                }
                spans.push(Span::styled(
                    app.input_buffer.as_str(),
                    Style::default().fg(Color::Rgb(230, 230, 230)),
                ));
                f.render_widget(Paragraph::new(Line::from(spans)), input_area);

                app.clamp_input_cursor();
                let cursor_x = input_area.x
                    + UnicodeWidthStr::width(icon_prefix.as_str()) as u16
                    + app.input_cursor as u16;
                let cursor_y = input_area.y;
                f.set_cursor(cursor_x.min(input_area.x + input_area.width.saturating_sub(1)), cursor_y);
            } else if matches!(app.mode, AppMode::DownloadInput | AppMode::DownloadNaming | AppMode::PasteRenaming | AppMode::ArchiveCreate | AppMode::NoteEditing | AppMode::CommandInput | AppMode::GitCommitMessage | AppMode::GitTagInput) {
                let area = f.size();
                let rename_area = Rect::new(area.width/4, area.height/2 - 1, area.width/2, 3);
                f.render_widget(Clear, rename_area);
                let title = match app.mode {
                    AppMode::DownloadInput => " Download URL (w: URL [name], quote URL if needed) ",
                    AppMode::DownloadNaming => " Save Download As ",
                    AppMode::PasteRenaming => " Paste As ",
                    AppMode::NewFile => " New File Name ",
                    AppMode::NewFolder => " New Folder Name ",
                    AppMode::ArchiveCreate => " Create Archive (Enter=Confirm, Esc=Cancel) ",
                    AppMode::NoteEditing => " Note (Enter=Save, Esc=Cancel) ",
                    AppMode::CommandInput => " Command (; Enter=Run, Esc=Cancel) ",
                    AppMode::GitCommitMessage => " Commit Message (Enter=Commit+Push, Esc=Cancel) ",
                    AppMode::GitTagInput => " Tag (Enter=Create+Push Tag, Esc=Cancel) ",
                    _ => " New Name ",
                };
                let prompt_value = app.input_buffer.clone();
                f.render_widget(Paragraph::new(prompt_value).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(title).title_style(Style::default().fg(Color::White))), rename_area);
                app.clamp_input_cursor();
                let cursor_x = rename_area.x + 1 + app.input_cursor as u16;
                let cursor_y = rename_area.y + 1;
                f.set_cursor(cursor_x.min(rename_area.x + rename_area.width.saturating_sub(1)), cursor_y);
            } else if app.mode == AppMode::ConfirmDownloadOverwrite {
                let area = f.size();
                let file_name = app
                    .download_pending_name
                    .as_deref()
                    .unwrap_or("download");
                let lines = vec![
                    "Overwrite existing file?".to_string(),
                    String::new(),
                    format!(" {}", file_name),
                    String::new(),
                    " y / Enter = overwrite    n / Esc = cancel".to_string(),
                ];
                let msg = lines.join("\n");
                let content_w = lines
                    .iter()
                    .map(|line| line.chars().count() as u16)
                    .max()
                    .unwrap_or(28);
                let dialog_w = (content_w + 2).max(40).min(area.width.saturating_sub(4).max(1));
                let dialog_h = (lines.len() as u16 + 2).max(7).min(area.height.saturating_sub(4).max(1));
                let confirm_area = Rect::new(
                    (area.width.saturating_sub(dialog_w)) / 2,
                    (area.height.saturating_sub(dialog_h)) / 2,
                    dialog_w,
                    dialog_h,
                );
                f.render_widget(Clear, confirm_area);
                f.render_widget(
                    Paragraph::new(msg)
                        .wrap(Wrap { trim: true })
                        .style(Style::default().fg(Color::Rgb(140, 200, 255)))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .title(" Confirm Download Overwrite ")
                                .title_style(Style::default().fg(Color::White)),
                        ),
                    confirm_area,
                );
            } else if app.mode == AppMode::Bookmarks {
                let bookmarks = App::load_bookmarks();
                if !bookmarks.is_empty() && app.bookmark_selected >= bookmarks.len() {
                    app.bookmark_selected = bookmarks.len() - 1;
                }
                ui::panels::render_bookmarks_overlay(
                    f,
                    tab_overlay_anchor,
                    app.panel_tab,
                    &bookmarks,
                    app.bookmark_selected,
                );
            } else if app.mode == AppMode::Integrations {
                let area = f.size();
                if !app.integration_rows_cache.is_empty()
                    && app.integration_selected >= app.integration_rows_cache.len()
                {
                    app.integration_selected = app.integration_rows_cache.len() - 1;
                }

                ui::panels::render_integrations_overlay(
                    f,
                    area,
                    tab_overlay_anchor,
                    app.panel_tab,
                    &app.integration_rows_cache,
                    app.integration_selected,
                );
            } else if app.mode == AppMode::SortMenu {
                let options = App::sort_mode_options();
                ui::panels::render_sort_overlay(
                    f,
                    tab_overlay_anchor,
                    app.panel_tab,
                    &options,
                    app.sort_menu_selected,
                    app.sort_mode,
                    app.nerd_font_active,
                );
            } else if app.mode == AppMode::SshPicker {
                let ssh_popup_w = tab_overlay_anchor.width;
                let ssh_content_w = ssh_popup_w.saturating_sub(2) as usize;
                let content_w = ssh_popup_w.saturating_sub(4) as usize;
                let type_w = 6usize;
                let mounted_w = 10usize;
                let available_for_alias_and_detail = content_w.saturating_sub(type_w + mounted_w + 3);
                let alias_w = if available_for_alias_and_detail >= 12 {
                    available_for_alias_and_detail.min(22)
                } else {
                    available_for_alias_and_detail
                };
                let detail_w = available_for_alias_and_detail.saturating_sub(alias_w);
                let trunc = |s: &str, max: usize| -> String {
                    if max == 0 {
                        return String::new();
                    }
                    if s.chars().count() <= max {
                        return s.to_string();
                    }
                    if max == 1 {
                        return "…".to_string();
                    }
                    let mut out = String::new();
                    for ch in s.chars().take(max - 1) {
                        out.push(ch);
                    }
                    out.push('…');
                    out
                };

                let mut lines: Vec<Line> = vec![Line::from("")];
                if app.remote_entries.is_empty() {
                    lines.push(Line::from(Span::styled(" No SSH/rclone/media mounts or mounted archives found", Style::default().fg(Color::Rgb(180, 80, 80)))));
                } else {
                    let mounted_aliases: HashSet<String> = app.ssh_mounts
                        .iter()
                        .map(|m| m._host_alias.clone())
                        .collect();
                    for (i, entry) in app.remote_entries.iter().enumerate() {
                        let is_selected = i == app.ssh_picker_selection;
                        let is_mounted = match entry {
                            RemoteEntry::ArchiveMount { .. } | RemoteEntry::LocalMount { .. } => true,
                            _ => mounted_aliases.contains(entry.alias()),
                        };
                        let mount_tag = if is_mounted { "  \u{25cf} mounted" } else { "" };
                        let (type_tag, detail) = match entry {
                            RemoteEntry::Ssh(h) => {
                                let user_at_host = match &h.user {
                                    Some(u) => format!("{}@{}", u, h.hostname),
                                    None => h.hostname.clone(),
                                };
                                let port_str = h.port.map(|p| format!(":{}", p)).unwrap_or_default();
                                ("ssh", format!("{}{}", user_at_host, port_str))
                            }
                            RemoteEntry::Rclone { rtype, .. } => ("rclone", rtype.clone()),
                            RemoteEntry::ArchiveMount { mount_path, .. } => ("zip", mount_path.to_string_lossy().into_owned()),
                            RemoteEntry::LocalMount { mount_path, source, .. } => ("mount", format!("{}: {}", source, mount_path.to_string_lossy())),
                        };
                        let type_col = format!("{:<width$}", type_tag, width = type_w);
                        let alias_col = format!(
                            "{:<width$}",
                            trunc(entry.alias(), alias_w),
                            width = alias_w
                        );
                        let detail_col = trunc(&detail, detail_w);
                        let label = format!(" {} {} {}{}", type_col, alias_col, detail_col, mount_tag);
                        let label = if is_selected {
                            let used_w = UnicodeWidthStr::width(label.as_str());
                            if ssh_content_w > used_w {
                                format!("{}{}", label, " ".repeat(ssh_content_w - used_w))
                            } else {
                                label
                            }
                        } else {
                            label
                        };
                        let style = if is_selected {
                            Style::default().fg(Color::White).bg(Color::Rgb(60, 60, 60)).add_modifier(Modifier::BOLD)
                        } else if is_mounted {
                            Style::default().fg(Color::Rgb(80, 220, 160))
                        } else {
                            Style::default().fg(Color::Rgb(200, 200, 200))
                        };
                        lines.push(Line::from(Span::styled(label, style)));
                    }
                }
                let ssh_h = (lines.len() as u16 + 4).max(8).min(tab_overlay_anchor.height);
                let ssh_area = Rect::new(
                    tab_overlay_anchor.x,
                    tab_overlay_anchor.y,
                    ssh_popup_w,
                    ssh_h,
                );
                f.render_widget(Clear, ssh_area);
                let ssh_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(App::panel_tab_bar_line(app.panel_tab))
                    .title_style(Style::default().fg(Color::White))
                    .border_style(Style::default().fg(Color::Rgb(80, 200, 180)));
                let ssh_inner = ssh_block.inner(ssh_area);
                f.render_widget(ssh_block, ssh_area);
                f.render_widget(
                    Paragraph::new(Span::styled(
                        "x",
                        Style::default().fg(Color::Rgb(170, 170, 170)),
                    )),
                    App::tabbed_overlay_close_area(ssh_area),
                );
                let ssh_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(ssh_inner);
                f.render_widget(Paragraph::new(lines), ssh_chunks[0]);
                f.render_widget(
                    Paragraph::new(ui::panels::shortcut_footer_lines(&[
                        ("↑↓", "navigate"),
                        ("Enter/→", "open or mount"),
                        ("u/Delete", "unmount"),
                        ("Tab", "switch tabs"),
                        ("Esc", "close"),
                    ])),
                    ssh_chunks[1],
                );
            } else if app.mode == AppMode::ConfirmExtract {
                let area = f.size();
                let to_extract = &app.archive_extract_targets;
                let mut msg_lines: Vec<String> = vec!["Extract selected archives?".to_string(), String::new()];
                let max_list_rows = ((area.height.saturating_sub(10) as usize).min(14)).max(1);
                for (idx, path) in to_extract.iter().enumerate() {
                    if idx >= max_list_rows {
                        break;
                    }
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.to_string_lossy().into_owned());
                    msg_lines.push(format!(" - {}", name));
                }
                if to_extract.len() > max_list_rows {
                    let remaining = to_extract.len() - max_list_rows;
                    msg_lines.push(format!(" ... and {} more", remaining));
                }
                msg_lines.push(String::new());
                msg_lines.push("Each archive is extracted to its own folder".to_string());
                msg_lines.push("  y = confirm    n / Esc = cancel".to_string());
                let msg = msg_lines.join("\n");

                let content_w = msg_lines
                    .iter()
                    .map(|line| line.chars().count() as u16)
                    .max()
                    .unwrap_or(28);
                let content_h = msg_lines.len() as u16;
                let max_w = area.width.saturating_sub(4).max(1);
                let max_h = area.height.saturating_sub(4).max(1);
                let dialog_w = (content_w + 2)
                    .max(40)
                    .min(max_w);
                let dialog_h = (content_h + 2)
                    .max(7)
                    .min(max_h);
                let confirm_area = Rect::new(
                    (area.width.saturating_sub(dialog_w)) / 2,
                    (area.height.saturating_sub(dialog_h)) / 2,
                    dialog_w,
                    dialog_h,
                );
                f.render_widget(Clear, confirm_area);
                f.render_widget(
                    Paragraph::new(msg)
                        .wrap(Wrap { trim: true })
                        .style(Style::default().fg(Color::Rgb(140, 200, 255)))
                        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Confirm Extract ").title_style(Style::default().fg(Color::White))),
                    confirm_area,
                );
            } else if app.mode == AppMode::ConfirmIntegrationInstall {
                let area = f.size();
                let msg_lines = app.confirm_integration_install_msg_lines();
                let confirm_area = app.confirm_integration_install_dialog_area(area);
                ui::dialogs::render_confirm_integration_install_dialog(
                    f,
                    &msg_lines,
                    confirm_area,
                    app.confirm_integration_install_button_focus,
                    app.nerd_font_active,
                );
            } else if app.mode == AppMode::ConfirmDelete {
                let area = f.size();
                let to_delete = app.delete_targets();
                let (mut file_count, mut folder_count) = (0usize, 0usize);
                for path in &to_delete {
                    if path.is_dir() {
                        folder_count += 1;
                    } else {
                        file_count += 1;
                    }
                }
                let title = ui::dialogs::confirm_delete_title(file_count, folder_count);
                let delete_state = ui::dialogs::render_confirm_delete_dialog(
                    f,
                    area,
                    &title,
                    &to_delete,
                    app.confirm_delete_scroll_offset,
                    app.confirm_delete_button_focus == 0,
                    app.show_icons,
                    |path, path_is_symlink| {
                        App::icon_for_path(path, app.show_icons, app.nerd_font_active, path_is_symlink)
                    },
                );
                app.confirm_delete_max_offset = delete_state.max_offset;
                app.confirm_delete_scroll_offset = delete_state.clamped_offset;
            }

            // --- Footer ---
            let total_entries = app.entries.len();
            let selected_ordinal = if total_entries == 0 {
                0
            } else {
                app.selected_index.min(total_entries - 1) + 1
            };
            let mut left_status_parts = vec![format!("{}/{}", selected_ordinal, total_entries)];
            if !app.clipboard.is_empty() {
                left_status_parts.push(format!("Clipboard:{}", app.clipboard.len()));
            }
            let left_status = left_status_parts.join(" │ ");
            let right_status = "c:Copy v:paste m:Move r:Rename w:Web d:Del e:Edit s:Size o:Open-GUI f:Find `:preview h:Help q:Quit";
            let width = chunks[1].width as usize;
            let left_len = left_status.chars().count();
            let right_len = right_status.chars().count();

            let (gap, right_display) = if left_len + right_len <= width {
                (
                    " ".repeat(width.saturating_sub(left_len + right_len)),
                    right_status.to_string(),
                )
            } else {
                let available = width.saturating_sub(left_len + 1);
                let right_trimmed = if available == 0 {
                    String::new()
                } else {
                    let tail: String = right_status
                        .chars()
                        .rev()
                        .take(available)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    format!("{:>width$}", tail, width = available)
                };
                (" ".to_string(), right_trimmed)
            };

            let mut left_spans: Vec<Span> = vec![
                Span::styled(selected_ordinal.to_string(), Style::default().fg(Color::White)),
                Span::styled("/", Style::default().fg(Color::DarkGray)),
                Span::styled(total_entries.to_string(), Style::default().fg(Color::White)),
            ];
            if !app.clipboard.is_empty() {
                left_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
                left_spans.push(Span::styled("Clipboard", Style::default().fg(Color::DarkGray)));
                left_spans.push(Span::styled(":", Style::default().fg(Color::DarkGray)));
                left_spans.push(Span::styled(app.clipboard.len().to_string(), Style::default().fg(Color::White)));
            }

            let mut right_spans: Vec<Span> = Vec::new();
            let mut segment = String::new();
            let mut in_ws = true;
            for ch in right_display.chars() {
                let is_ws = ch.is_whitespace();
                if segment.is_empty() {
                    in_ws = is_ws;
                }
                if is_ws == in_ws {
                    segment.push(ch);
                } else {
                    if in_ws {
                        right_spans.push(Span::styled(segment.clone(), Style::default().fg(Color::DarkGray)));
                    } else if let Some(colon_idx) = segment.find(':') {
                        let (key, rest) = segment.split_at(colon_idx);
                        if !key.is_empty() {
                            right_spans.push(Span::styled(key.to_string(), Style::default().fg(Color::White)));
                        }
                        right_spans.push(Span::styled(rest.to_string(), Style::default().fg(Color::DarkGray)));
                    } else {
                        right_spans.push(Span::styled(segment.clone(), Style::default().fg(Color::DarkGray)));
                    }
                    segment.clear();
                    segment.push(ch);
                    in_ws = is_ws;
                }
            }
            if !segment.is_empty() {
                if in_ws {
                    right_spans.push(Span::styled(segment, Style::default().fg(Color::DarkGray)));
                } else if let Some(colon_idx) = segment.find(':') {
                    let (key, rest) = segment.split_at(colon_idx);
                    if !key.is_empty() {
                        right_spans.push(Span::styled(key.to_string(), Style::default().fg(Color::White)));
                    }
                    right_spans.push(Span::styled(rest.to_string(), Style::default().fg(Color::DarkGray)));
                } else {
                    right_spans.push(Span::styled(segment, Style::default().fg(Color::DarkGray)));
                }
            }

            let mut status_spans: Vec<Span> = left_spans;
            status_spans.push(Span::raw(gap));
            status_spans.extend(right_spans);
            let status = Line::from(status_spans);
            let footer_block = Block::default()
                .borders(Borders::TOP)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray));
            f.render_widget(Paragraph::new(status).block(footer_block), chunks[1]);
            let selected_total_status = if app.copy_rx.is_none() && app.archive_rx.is_none() {
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
                let lower_msg = status_text.to_ascii_lowercase();
                let is_error = lower_msg.contains("error")
                    || lower_msg.contains("failed")
                    || lower_msg.contains("not found")
                    || lower_msg.contains("refresh failed");
                let msg_style = if selected_total_is_shown {
                    Style::default().fg(Color::Rgb(150, 220, 150))
                } else if app.copy_rx.is_some() || app.archive_rx.is_some() {
                    Style::default().fg(Color::Rgb(120, 200, 255))
                } else if is_error {
                    Style::default().fg(Color::Rgb(255, 120, 120))
                } else {
                    Style::default().fg(Color::White)
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
                    ui::scrollbar::render_scrollbar_corners(f, table_area, can_draw_scrollbar, Color::DarkGray);
                }
            }
        })?;

        // After ratatui has drawn, overlay native protocol image in the preview pane
        // for terminals that support in-pane rendering.
        let native_protocol = App::terminal_image_protocol().0;
        let native_pane_supported = matches!(
            native_protocol,
            crate::integration::probe::TerminalImageProtocol::Kitty
                | crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel
        );
        if native_pane_supported && app.is_preview_mode() {
            if let (Some(area), Some(ref png), Some((rgb, iw, ih))) = (
                app.preview_native_area,
                app.preview_image_png.as_ref(),
                app.preview_image_rgb.as_ref(),
            ) {
                let fit = App::fit_native_image_area(area, *iw, *ih);
                let path_key = app
                    .preview_target_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "<no-path>".to_string());
                let draw_key = format!(
                    "{}|{}x{}|{}:{}:{}:{}",
                    path_key, iw, ih, fit.x, fit.y, fit.width, fit.height
                );

                if app.preview_native_last_key.as_deref() != Some(draw_key.as_str()) {
                    match native_protocol {
                        crate::integration::probe::TerminalImageProtocol::Kitty => {
                            let _ = App::clear_kitty_pane_images();
                            let _ = App::emit_kitty_pane(
                                png,
                                *iw,
                                *ih,
                                fit.x,
                                fit.y,
                                fit.width,
                                fit.height,
                            );
                        }
                        crate::integration::probe::TerminalImageProtocol::Iterm2Inline => {
                            // Use the full preview pane bounds so clearing removes
                            // remnants from previously larger images.
                            let _ = App::emit_iterm2_pane(
                                png,
                                area.x,
                                area.y,
                                area.width,
                                area.height,
                            );
                        }
                        crate::integration::probe::TerminalImageProtocol::Sixel => {
                            // Pass the full pane area: emit_sixel_pane handles its
                            // own pixel-aware sizing and clears stale content first.
                            let _ = App::emit_sixel_pane(
                                rgb,
                                *iw,
                                *ih,
                                area.x,
                                area.y,
                                area.width,
                                area.height,
                            );
                        }
                        _ => {}
                    }
                    app.preview_native_last_key = Some(draw_key);
                }
            } else if app.preview_native_last_key.is_some() {
                // Switched from image -> non-image (folder/text/etc.): clear once.
                match native_protocol {
                    crate::integration::probe::TerminalImageProtocol::Kitty => {
                        let _ = App::clear_kitty_pane_images();
                    }
                    crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                    | crate::integration::probe::TerminalImageProtocol::Sixel => {
                        if let Some(area) = app.preview_native_area {
                            let _ = App::clear_preview_pane_area(
                                area.x,
                                area.y,
                                area.width,
                                area.height,
                            );
                        }
                    }
                    _ => {}
                }
                app.preview_native_last_key = None;
            }
        } else if app.preview_native_last_key.is_some() {
            // Preview disabled (or no longer native pane): clear once and stop tracking.
            match native_protocol {
                crate::integration::probe::TerminalImageProtocol::Kitty => {
                    let _ = App::clear_kitty_pane_images();
                }
                crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel => {
                    if let Some(area) = app.preview_native_area {
                        let _ = App::clear_preview_pane_area(
                            area.x,
                            area.y,
                            area.width,
                            area.height,
                        );
                    }
                }
                _ => {}
            }
            app.preview_native_last_key = None;
        }

        let mut next_key: Option<KeyEvent> = deferred_key.take();
        if next_key.is_none() && event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) => {
                    next_key = Some(key);
                }
                Event::Mouse(mouse) => {
                    let area = terminal.size()?;
                    if let Some(simulated_key) = app.handle_mouse_event(mouse, area) {
                        deferred_key = Some(simulated_key);
                    }
                    continue;
                }
                _ => {}
            }
        }

        if let Some(key) = next_key {
            match key_dispatch::handle_app_key_event(terminal, app, key, &mut deferred_key)? {
                key_dispatch::KeyDispatchOutcome::Quit => break,
                key_dispatch::KeyDispatchOutcome::ContinueLoop => continue,
                key_dispatch::KeyDispatchOutcome::Ok => {}
            }
        }
    }

    Ok(())

}
