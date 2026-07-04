use super::*;

pub(crate) fn render_overlays(f: &mut Frame, app: &mut App, ctx: &RenderCtx) {
    let active_theme = ctx.theme;
    let chunks = [ctx.main, ctx.footer];

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
        render_internal_search_overlay(f, app, ctx, tab_overlay_anchor);
    } else if app.mode == AppMode::DbPreview {
        render_db_preview_overlay(f, app, ctx, tab_overlay_anchor);
    } else if app.mode == AppMode::Help {
        let (max_off, clamped_off, logo_area) = ui::panels::render_help_overlay(
            f,
            tab_overlay_anchor,
            app.panel_tab,
            app.active_theme,
            app.help_scroll_offset,
            app.nerd_font_active,
            &app.keymap,
            &mut app.footer_shortcut_zones,
        );
        app.help_max_offset = max_off;
        app.help_scroll_offset = clamped_off;
        app.help_logo_native_area = logo_area;
    } else if matches!(app.mode, AppMode::NewFile | AppMode::NewFolder) {
        render_new_entry_overlay(f, app, ctx);
    } else if app.mode == AppMode::Renaming {
        let area = f.size();
        let selected_entry = app.left.entries.get(app.left.selected_index);
        let old_name = selected_entry
            .map(crate::util::classify::entry_name)
            .unwrap_or_else(|| app.input_buffer.clone());
        let selected_path = selected_entry.map(|e| e.path());
        let selected_is_dir = selected_path.as_ref().map(|p| p.is_dir()).unwrap_or(false);
        let selected_is_symlink = selected_path
            .as_ref()
            .map(crate::util::classify::is_symlink)
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
            .title_style(Style::default().fg(active_theme.text_normal))
            .border_style(Style::default().fg(active_theme.border));
        let input_area = block.inner(rename_area);
        f.render_widget(block, rename_area);

        let (icon_glyph, icon_style) = App::icon_for_name(
            app.input_buffer.as_str(),
            selected_is_dir,
            app.show_icons,
            app.nerd_font_active,
            selected_is_symlink,
            app.active_theme,
        );
        let icon_prefix = if app.show_icons && !icon_glyph.is_empty() {
            format!("{} ", icon_glyph)
        } else {
            String::new()
        };
        app.clamp_input_cursor();
        let icon_w = UnicodeWidthStr::width(icon_prefix.as_str()) as usize;
        let avail_w = (input_area.width as usize).saturating_sub(icon_w);
        let cursor = app.input_cursor;
        let scroll = if avail_w > 0 && cursor >= avail_w { cursor + 1 - avail_w } else { 0 };
        let visible_text: String = app.input_buffer.chars().skip(scroll).collect();
        let mut spans = Vec::new();
        if !icon_prefix.is_empty() {
            spans.push(Span::styled(icon_prefix.clone(), icon_style));
        }
        spans.push(Span::styled(
            visible_text,
            Style::default().fg(active_theme.text_normal),
        ));
        f.render_widget(Paragraph::new(Line::from(spans)), input_area);

        let cursor_x = input_area.x
            + UnicodeWidthStr::width(icon_prefix.as_str()) as u16
            + (cursor - scroll) as u16;
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
            AppMode::GitCommitMessage => " Commit Message (Enter=Commit+Push, Ctrl+G=AI, Esc=Cancel) ",
            AppMode::GitTagInput => " Tag (Enter=Create+Push Tag, Esc=Cancel) ",
            _ => " New Name ",
        };
        app.clamp_input_cursor();
        let avail_w = (rename_area.width as usize).saturating_sub(2);
        let cursor = app.input_cursor;
        let scroll = if avail_w > 0 && cursor >= avail_w { cursor + 1 - avail_w } else { 0 };
        let visible_text: String = app.input_buffer.chars().skip(scroll).collect();
        f.render_widget(Paragraph::new(visible_text).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(title).title_style(Style::default().fg(active_theme.text_normal))), rename_area);
        let cursor_x = rename_area.x + 1 + (cursor - scroll) as u16;
        let cursor_y = rename_area.y + 1;
        f.set_cursor(cursor_x.min(rename_area.x + rename_area.width.saturating_sub(1)), cursor_y);
    } else if app.mode == AppMode::ConfirmDownloadOverwrite {
        let area = f.size();
        let file_name = app
            .download_pending_name
            .as_deref()
            .unwrap_or("download");
        let lines = ["Overwrite existing file?".to_string(),
            String::new(),
            format!(" {}", file_name),
            String::new(),
            " y / Enter = overwrite    n / Esc = cancel".to_string()];
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
                .style(Style::default().fg(active_theme.overlay_section))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(" Confirm Download Overwrite ")
                        .title_style(Style::default().fg(active_theme.text_normal)),
                ),
            confirm_area,
        );
    } else if app.mode == AppMode::Bookmarks || app.mode == AppMode::BookmarkEditing || app.mode == AppMode::ConfirmDeleteBookmark {
        let bookmarks = app.bookmarks().to_vec();
        if !bookmarks.is_empty() && app.bookmark_selected >= bookmarks.len() {
            app.bookmark_selected = bookmarks.len() - 1;
        }
        ui::panels::render_bookmarks_overlay(
            f,
            ui::panels::OverlayChrome {
                anchor: tab_overlay_anchor,
                panel_tab: app.panel_tab,
                theme_id: app.active_theme,
                nerd_font: app.nerd_font_active,
            },
            &bookmarks,
            app.bookmark_selected,
            &mut app.footer_shortcut_zones,
        );
        if app.mode == AppMode::BookmarkEditing {
            let area = f.size();
            let rename_area = Rect::new(area.width / 4, area.height / 2 - 1, area.width / 2, 3);
            f.render_widget(Clear, rename_area);
            let title = format!(" Set Bookmark {} (Enter=Save, Esc=Cancel) ", app.bookmark_edit_idx);
            app.clamp_input_cursor();
            let avail_w = (rename_area.width as usize).saturating_sub(2);
            let cursor = app.input_cursor;
            let scroll = if avail_w > 0 && cursor >= avail_w { cursor + 1 - avail_w } else { 0 };
            let visible_text: String = app.input_buffer.chars().skip(scroll).collect();
            f.render_widget(
                Paragraph::new(visible_text).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(title.as_str())
                        .title_style(Style::default().fg(active_theme.text_normal)),
                ),
                rename_area,
            );
            let cursor_x = rename_area.x + 1 + (cursor - scroll) as u16;
            f.set_cursor(
                cursor_x.min(rename_area.x + rename_area.width.saturating_sub(1)),
                rename_area.y + 1,
            );
        } else if app.mode == AppMode::ConfirmDeleteBookmark {
            let area = f.size();
            let bm_idx = app.bookmark_delete_idx;
            let path_str = app
                .bookmarks()
                .iter()
                .find(|(i, _)| *i == bm_idx)
                .and_then(|(_, p)| p.as_ref())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let from_env = std::env::var(format!("SB_BOOKMARK_{}", bm_idx)).is_ok();
            ui::dialogs::render_confirm_delete_bookmark_dialog(
                f,
                area,
                &ui::dialogs::ConfirmDeleteBookmarkView {
                    bookmark_idx: bm_idx,
                    bookmark_path: &path_str,
                    from_env,
                    button_focus: app.confirm_delete_bookmark_button_focus,
                    nerd_font_active: app.nerd_font_active,
                    theme: &active_theme,
                },
            );
        }
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
            ui::panels::OverlayChrome {
                anchor: tab_overlay_anchor,
                panel_tab: app.panel_tab,
                theme_id: app.active_theme,
                nerd_font: app.nerd_font_active,
            },
            ui::panels::IntegrationsOverlayState {
                integrations: &app.integration_rows_cache,
                integration_selected: app.integration_selected,
                search_active: app.integration_search_active,
                search_query: &app.integration_search_query,
                show_icons: app.show_icons,
            },
            &mut app.footer_shortcut_zones,
        );
    } else if app.mode == AppMode::Themes {
        ui::panels::render_themes_overlay(
            f,
            ui::panels::OverlayChrome {
                anchor: tab_overlay_anchor,
                panel_tab: app.panel_tab,
                theme_id: app.active_theme,
                nerd_font: app.nerd_font_active,
            },
            ui::panels::ThemesOverlayState {
                selected: app.theme_selected,
                nerd_focus: app.theme_panel_nerd_selected,
                color_mode: app.filename_color_mode,
                color_focus: app.theme_panel_color_selected,
                disable_clock: app.disable_clock,
                clock_focus: app.theme_panel_clock_selected,
            },
            &mut app.footer_shortcut_zones,
        );
    } else if app.mode == AppMode::SortMenu {
        let options = App::sort_mode_options();
        ui::panels::render_sort_overlay(
            f,
            ui::panels::OverlayChrome {
                anchor: tab_overlay_anchor,
                panel_tab: app.panel_tab,
                theme_id: app.active_theme,
                nerd_font: app.nerd_font_active,
            },
            &options,
            app.sort_menu_selected,
            app.left.sort_mode,
            &mut app.footer_shortcut_zones,
        );
    } else if app.mode == AppMode::Settings {
        let provider = crate::app_ai::provider_by_key(&app.ai_provider);
        let model_is_default = app.ai_model.trim().is_empty();
        let model_display = app.resolve_ai_model();
        let model_value = if model_is_default {
            format!("{} (default)", model_display)
        } else {
            model_display
        };
        let key_value = if !app.ai_api_key.trim().is_empty() {
            "•".repeat(app.ai_api_key.chars().count().min(24))
        } else if std::env::var(provider.env_var).map(|v| !v.trim().is_empty()).unwrap_or(false) {
            format!("(from ${})", provider.env_var)
        } else {
            "(not set)".to_string()
        };
        let key_is_fallback = app.ai_api_key.trim().is_empty();
        // Only show a validation glyph once the user has actually set a key.
        let key_status = if key_is_fallback {
            ui::panels::SettingsRowStatus::None
        } else {
            match app.ai_key_status {
                crate::AiKeyStatus::Checking => ui::panels::SettingsRowStatus::Checking,
                crate::AiKeyStatus::Valid => ui::panels::SettingsRowStatus::Valid,
                crate::AiKeyStatus::Invalid => ui::panels::SettingsRowStatus::Invalid,
                crate::AiKeyStatus::Unknown => ui::panels::SettingsRowStatus::None,
            }
        };
        use ui::panels::SettingsRowStatus;
        let rows = [
            ui::panels::SettingsRow { label: "Provider", value: provider.label, dim_value: false, status: SettingsRowStatus::None },
            ui::panels::SettingsRow { label: "Model", value: &model_value, dim_value: model_is_default, status: SettingsRowStatus::None },
            ui::panels::SettingsRow { label: "API Key", value: &key_value, dim_value: key_is_fallback, status: key_status },
            ui::panels::SettingsRow { label: "Auto AI commit message", value: if app.ai_auto_commit { "On" } else { "Off" }, dim_value: !app.ai_auto_commit, status: SettingsRowStatus::None },
        ];
        ui::panels::render_settings_overlay(
            f,
            ui::panels::OverlayChrome {
                anchor: tab_overlay_anchor,
                panel_tab: app.panel_tab,
                theme_id: app.active_theme,
                nerd_font: app.nerd_font_active,
            },
            &rows,
            app.settings_selected,
            &mut app.footer_shortcut_zones,
        );
    } else if app.mode == AppMode::Shortcuts {
        ui::panels::render_shortcuts_overlay(
            f,
            ui::panels::OverlayChrome {
                anchor: tab_overlay_anchor,
                panel_tab: app.panel_tab,
                theme_id: app.active_theme,
                nerd_font: app.nerd_font_active,
            },
            &app.keymap,
            app.shortcuts_selected,
            app.shortcut_capture,
            &mut app.footer_shortcut_zones,
        );
    } else if app.mode == AppMode::SshPicker {
        render_ssh_picker_overlay(f, app, ctx, tab_overlay_anchor);
    } else if app.mode == AppMode::ConfirmExtract {
        let area = f.size();
        let to_extract = &app.archive.extract_targets;
        let mut msg_lines: Vec<String> = vec!["Extract selected archives?".to_string(), String::new()];
        let max_list_rows = (area.height.saturating_sub(10) as usize).clamp(1, 14);
        for (idx, path) in to_extract.iter().enumerate() {
            if idx >= max_list_rows {
                break;
            }
            let name = crate::util::classify::display_name(path.as_path());
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
                .style(Style::default().fg(active_theme.overlay_section))
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Confirm Extract ").title_style(Style::default().fg(active_theme.text_normal))),
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
            &active_theme,
        );
    } else if app.mode == AppMode::ConfirmDelete {
        let area = f.size();
        let to_delete = &app.confirm_delete_targets;
        let folder_count = to_delete.iter().filter(|t| t.is_dir).count();
        let file_count = to_delete.len() - folder_count;
        let title = ui::dialogs::confirm_delete_title(file_count, folder_count);
        let delete_state = ui::dialogs::render_confirm_delete_dialog(
            f,
            area,
            &ui::dialogs::ConfirmDeleteView {
                title: &title,
                to_delete,
                scroll_offset: app.confirm_delete_scroll_offset,
                confirm_focused: app.confirm_delete_button_focus == 0,
                show_icons: app.show_icons,
                nerd_font_active: app.nerd_font_active,
                theme: &active_theme,
            },
            |path, path_is_symlink| {
                App::icon_for_path(path, app.show_icons, app.nerd_font_active, path_is_symlink, app.active_theme)
            },
        );
        app.confirm_delete_max_offset = delete_state.max_offset;
        app.confirm_delete_scroll_offset = delete_state.clamped_offset;
    } else if app.mode == AppMode::Organize {
        let area = f.size();
        match &app.organize_plan {
            Some(plan) => {
                let title = format!(
                    " Organize Preview ({} folder(s), {} move(s)) ",
                    plan.folders.len(),
                    plan.moves.len()
                );
                let organize_state = ui::dialogs::render_organize_plan_dialog(
                    f,
                    area,
                    &ui::dialogs::OrganizePlanView {
                        title: &title,
                        folders: &plan.folders,
                        moves: &plan.moves,
                        scroll_offset: app.organize_scroll_offset,
                        confirm_focused: app.organize_button_focus == 0,
                        nerd_font_active: app.nerd_font_active,
                        theme: &active_theme,
                    },
                );
                app.organize_max_offset = organize_state.max_offset;
                app.organize_scroll_offset = organize_state.clamped_offset;
            }
            None => {
                let title = " Organize ";
                let msg = "Generating organize plan...";
                let dialog_w = (msg.len() as u16 + 6).max(36).min(area.width.saturating_sub(4));
                let dialog_h = 5u16.min(area.height.saturating_sub(4));
                let box_area = Rect::new(
                    (area.width.saturating_sub(dialog_w)) / 2,
                    (area.height.saturating_sub(dialog_h)) / 2,
                    dialog_w,
                    dialog_h,
                );
                f.render_widget(Clear, box_area);
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(title)
                    .title_style(Style::default().fg(active_theme.text_normal))
                    .border_style(Style::default().fg(active_theme.accent_primary));
                let inner = block.inner(box_area);
                f.render_widget(block, box_area);
                f.render_widget(
                    Paragraph::new(msg).alignment(Alignment::Center),
                    inner,
                );
            }
        }
    }

}

pub(crate) fn render_internal_search_overlay(f: &mut Frame, app: &mut App, ctx: &RenderCtx, tab_overlay_anchor: Rect) {
    let active_theme = ctx.theme;
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
            .title(App::panel_tab_bar_line(app.panel_tab, app.active_theme, app.nerd_font_active, popup_area.width.saturating_sub(3)))
            .title_style(Style::default().fg(active_theme.text_normal))
            .style(Style::default().bg(active_theme.bg_panel).fg(active_theme.text_normal))
            .border_style(Style::default().fg(active_theme.divider));
        let popup_inner = popup_block.inner(popup_area);
        f.render_widget(popup_block, popup_area);
        f.render_widget(
            Paragraph::new(Span::styled(
                "x",
                Style::default().fg(active_theme.text_normal),
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
            .border_style(Style::default().fg(active_theme.border));
        let query_inner = query_box_block.inner(query_box_area);
        f.render_widget(query_box_block, query_box_area);

        let (mode_text, mode_style) = if app.search.scope == InternalSearchScope::Content {
            (
                "Scope: Content".to_string(),
                Style::default().fg(active_theme.success),
            )
        } else {
            (
                "Scope: Filename".to_string(),
                Style::default().fg(active_theme.accent_primary),
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
            Span::styled(query_icon_prefix.clone(), Style::default().fg(active_theme.accent_primary)),
            Span::styled(app.input_buffer.as_str(), Style::default().fg(active_theme.key_label)),
        ]);
        f.render_widget(Paragraph::new(query_line), query_input_area);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(mode_text.clone(), mode_style))).alignment(Alignment::Right),
            query_mode_area,
        );

        let mut lines: Vec<Line> = Vec::new();

        if app.search.candidates_pending {
            lines.push(Line::from(Span::styled(
                "Indexing files asynchronously...",
                Style::default().fg(active_theme.overlay_section),
            )));
        } else if app.search.candidates_truncated {
            lines.push(Line::from(Span::styled(
                "Indexed first 20000 files (refine query to narrow results)",
                Style::default().fg(active_theme.text_dim),
            )));
        }

        if app.search.scope == InternalSearchScope::Content {
            let limits = app.search.content_limits;
            lines.push(Line::from(Span::styled(
                format!(
                    " Limits: files={}  hits={}  max-file={}",
                    limits.max_files,
                    limits.max_hits,
                    App::format_size(limits.max_file_bytes as u64)
                ),
                Style::default().fg(active_theme.text_dim),
            )));

            if app.search.limits_menu_open {
                let selected_style = Style::default().fg(active_theme.key_label).add_modifier(Modifier::BOLD);
                let normal_style = Style::default().fg(active_theme.text_dim);
                let item_line = |idx: usize, label: &str, value: String| {
                    let marker = if idx == app.search.limits_selected { ">" } else { " " };
                    let style = if idx == app.search.limits_selected {
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
                    Style::default().fg(active_theme.text_dim),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    " Ctrl+L open limits editor",
                    Style::default().fg(active_theme.text_dim),
                )));
            }

            if app.search.content_pending {
                lines.push(Line::from(Span::styled(
                    " Scanning content asynchronously...",
                    Style::default().fg(active_theme.overlay_section),
                )));
            }
            if let Some(note) = &app.search.content_limit_note {
                lines.push(Line::from(Span::styled(
                    note.clone(),
                    Style::default().fg(active_theme.text_dim),
                )));
            }
        }

        let selected = app.search.selected;
        let body_content_w = body_area.width as usize;
        let visible_rows = body_area.height as usize;
        let header_rows = lines.len();
        let max_rows = visible_rows.saturating_sub(header_rows).max(1);
        let offset = if selected >= max_rows {
            selected + 1 - max_rows
        } else {
            0
        };
        let search_total_rows = app.search.results.len();
        let search_max_scroll = search_total_rows.saturating_sub(max_rows);
        let search_scroll_offset = offset.min(search_max_scroll);
        let can_draw_search_scrollbar = body_area.width > 2 && search_total_rows > max_rows;

        if let Some(err) = &app.search.regex_error {
            lines.push(Line::from(Span::styled(
                format!("Regex error: {}", err),
                Style::default().fg(active_theme.error),
            )));
        }

        if app.search.results.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " No matches",
                Style::default().fg(active_theme.error),
            )));
        } else {
            for (display_idx, result_idx) in app
                .search.results
                .iter()
                .skip(offset)
                .take(max_rows)
                .enumerate()
            {
                let absolute_idx = offset + display_idx;
                let is_selected = absolute_idx == selected;
                let row_inner_w = body_content_w.saturating_sub(2);
                let (left_cap, right_cap) = if is_selected {
                    if app.nerd_font_active {
                        (
                            Span::styled(
                                "",
                                Style::default()
                                    .fg(active_theme.bg_selected)
                                    .bg(active_theme.bg_panel),
                            ),
                            Span::styled(
                                "",
                                Style::default()
                                    .fg(active_theme.bg_selected)
                                    .bg(active_theme.bg_panel),
                            ),
                        )
                    } else {
                        (
                            Span::styled(" ", Style::default().bg(active_theme.bg_selected)),
                            Span::styled(" ", Style::default().bg(active_theme.bg_selected)),
                        )
                    }
                } else {
                    (
                        Span::styled(" ", Style::default().bg(active_theme.bg_panel)),
                        Span::styled(" ", Style::default().bg(active_theme.bg_panel)),
                    )
                };
                let base_style = if is_selected {
                    Style::default()
                        .fg(crate::ui::palette::readable_fg(active_theme.bg_selected, Color::Black, active_theme.text_normal))
                        .bg(active_theme.bg_selected)
                } else {
                    Style::default().fg(active_theme.text_normal)
                };
                let match_style = if is_selected {
                    Style::default()
                        .fg(active_theme.warning)
                        .bg(active_theme.bg_selected)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(active_theme.key_label)
                        .add_modifier(Modifier::BOLD)
                };
                let mut spans: Vec<Span> = vec![left_cap];

                let rel_path_for_icon = match result_idx {
                    InternalSearchResult::Filename { rel_path, .. } => rel_path,
                    InternalSearchResult::Content { rel_path, .. } => rel_path,
                };
                // Candidates are collected with `is_file()` (dirs are traversed,
                // not listed), so results are never directories; the symlink flag
                // was captured during the walk. No stat calls while drawing.
                let is_symlink = app.search.candidate_symlinks.contains(rel_path_for_icon.as_path());
                let is_dir = false;
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
                    app.active_theme,
                );
                let icon_span = if app.show_icons && !icon_glyph.is_empty() {
                    let adjusted_icon_style = if is_selected {
                        icon_style.bg(active_theme.bg_selected)
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
                                base_style.fg(active_theme.accent_primary),
                            ));
                        }
                        if let Some(icon) = icon_span {
                            spans.push(icon);
                        }
                        spans.push(Span::styled(
                            format!("{}:{}: ", base_part, line_number),
                            base_style.fg(active_theme.accent_primary),
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
                    if row_inner_w > used_w {
                        spans.push(Span::styled(
                            " ".repeat(row_inner_w - used_w),
                            base_style,
                        ));
                    }
                }
                spans.push(right_cap);

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
            ui::scrollbar::render_scrollbar_track(
                f,
                sb_area,
                search_total_rows,
                max_rows,
                search_scroll_offset,
                active_theme.divider,
                active_theme.divider,
            );
        }
        let search_footer_entries: &[(&'static str, &'static str)] = &[
            ("↑↓", "navigate"),
            ("Enter", "open"),
            ("Ctrl+T", "toggle scope"),
            ("Regex", "re:pattern or /pattern/i"),
            ("Tab", "switch tabs"),
        ];
        f.render_widget(
            Paragraph::new(ui::panels::shortcut_footer_lines(
                search_footer_entries,
                app.active_theme,
                app.nerd_font_active,
            )),
            footer_area,
        );
        app.footer_shortcut_zones.extend(ui::panels::footer_shortcut_zones(
            search_footer_entries,
            footer_area,
            app.nerd_font_active,
        ));

        app.clamp_input_cursor();
        let cursor_x = query_input_area.x
            + UnicodeWidthStr::width(query_icon_prefix.as_str()) as u16
            + app.input_cursor as u16;
        let cursor_y = query_input_area.y;
        f.set_cursor(
            cursor_x.min(query_input_area.x + query_input_area.width.saturating_sub(1)),
            cursor_y,
        );
}

pub(crate) fn render_db_preview_overlay(f: &mut Frame, app: &mut App, ctx: &RenderCtx, tab_overlay_anchor: Rect) {
    let active_theme = ctx.theme;
        let popup_area = Rect::new(
            tab_overlay_anchor.x,
            tab_overlay_anchor.y,
            tab_overlay_anchor.width,
            tab_overlay_anchor.height,
        );

        let db_title = app
            .db_preview_path
            .as_ref()
            .and_then(|p| crate::util::classify::path_file_name(p))
            .unwrap_or_else(|| "SQLite Preview".to_string());

        let mut lines: Vec<Line> = vec![
            Line::from(Span::styled(
                "←→:switch table  Home/End:jump  Esc:close",
                Style::default().fg(active_theme.text_dim),
            )),
        ];

        let mut table_spans: Vec<Span> = vec![Span::styled(
            "Tables: ",
            Style::default().fg(active_theme.text_dim),
        )];
        if app.db_preview_tables.is_empty() {
            table_spans.push(Span::styled(
                "(none)",
                Style::default().fg(active_theme.error),
            ));
        } else {
            for (idx, table_name) in app.db_preview_tables.iter().enumerate() {
                if idx > 0 {
                    table_spans.push(Span::styled("  ", Style::default().fg(active_theme.text_dim)));
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
                        .bg(active_theme.success)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(active_theme.accent_primary)
                };
                table_spans.push(Span::styled(display, style));
            }
        }
        lines.push(Line::from(table_spans));

        if let Some(err) = &app.db_preview_error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                err.clone(),
                Style::default().fg(active_theme.error),
            )));
        } else {
            lines.push(Line::from(""));
            if app.db_preview_output_lines.is_empty() {
                lines.push(Line::from(Span::styled(
                    "(no rows)",
                    Style::default().fg(active_theme.text_dim),
                )));
            } else {
                let visible_w = popup_area.width.saturating_sub(4) as usize;
                for row in &app.db_preview_output_lines {
                    lines.push(Line::from(Span::styled(
                        truncate_with_ellipsis(row, visible_w),
                        Style::default().fg(active_theme.text_normal),
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
                        .title_style(Style::default().fg(active_theme.text_normal))
                        .border_style(Style::default().fg(active_theme.success)),
                )
                .wrap(Wrap { trim: true }),
            popup_area,
        );
}

pub(crate) fn render_new_entry_overlay(f: &mut Frame, app: &mut App, ctx: &RenderCtx) {
    let active_theme = ctx.theme;
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
            .title_style(Style::default().fg(active_theme.text_normal))
            .border_style(Style::default().fg(active_theme.border));
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
                app.active_theme,
            );
            let mut spans = Vec::new();
            if app.show_icons && !icon_glyph.is_empty() {
                spans.push(Span::styled(format!("{} ", icon_glyph), icon_style));
            }
            spans.push(Span::styled(*line, Style::default().fg(active_theme.text_normal)));
            rendered_lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(rendered_lines), list_area);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "(/name = folder, name = file)  Alt+Enter: new line",
                Style::default().fg(active_theme.text_dim),
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
            app.active_theme,
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
}

pub(crate) fn render_ssh_picker_overlay(f: &mut Frame, app: &mut App, ctx: &RenderCtx, tab_overlay_anchor: Rect) {
    let active_theme = ctx.theme;
        let ssh_popup_w = tab_overlay_anchor.width;
        let ssh_content_w = ssh_popup_w.saturating_sub(2) as usize;
        let ssh_row_inner_w = ssh_content_w.saturating_sub(2);
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
        let trunc = truncate_with_ellipsis;

        let mut lines: Vec<Line> = vec![Line::from("")];
        if app.remote_entries.is_empty() {
            lines.push(Line::from(Span::styled(" No SSH/rclone/media mounts or mounted archives found", Style::default().fg(active_theme.error))));
        } else {
            let mounted_aliases: HashSet<String> = app.ssh_mounts
                .iter()
                .map(|m| m.host_alias.clone())
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
                    if ssh_row_inner_w > used_w {
                        format!("{}{}", label, " ".repeat(ssh_row_inner_w - used_w))
                    } else {
                        label
                    }
                } else {
                    label
                };
                let style = if is_selected {
                    Style::default()
                        .fg(crate::ui::palette::readable_fg(active_theme.bg_selected, Color::Black, active_theme.text_normal))
                        .bg(active_theme.bg_selected)
                        .add_modifier(Modifier::BOLD)
                } else if is_mounted {
                    Style::default().fg(active_theme.success)
                } else {
                    Style::default().fg(active_theme.text_normal)
                };
                let (left_cap, right_cap) = if is_selected {
                    if app.nerd_font_active {
                        (
                            Span::styled(
                                "",
                                Style::default()
                                    .fg(active_theme.bg_selected)
                                    .bg(active_theme.bg_panel),
                            ),
                            Span::styled(
                                "",
                                Style::default()
                                    .fg(active_theme.bg_selected)
                                    .bg(active_theme.bg_panel),
                            ),
                        )
                    } else {
                        (
                            Span::styled(" ", Style::default().bg(active_theme.bg_selected)),
                            Span::styled(" ", Style::default().bg(active_theme.bg_selected)),
                        )
                    }
                } else {
                    (
                        Span::styled(" ", Style::default().bg(active_theme.bg_panel)),
                        Span::styled(" ", Style::default().bg(active_theme.bg_panel)),
                    )
                };
                lines.push(Line::from(vec![
                    left_cap,
                    Span::styled(label, style),
                    right_cap,
                ]));
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
            .title(App::panel_tab_bar_line(app.panel_tab, app.active_theme, app.nerd_font_active, ssh_area.width.saturating_sub(3)))
            .title_style(Style::default().fg(active_theme.text_normal))
            .style(Style::default().bg(active_theme.bg_panel).fg(active_theme.text_normal))
            .border_style(Style::default().fg(active_theme.divider));
        let ssh_inner = ssh_block.inner(ssh_area);
        f.render_widget(ssh_block, ssh_area);
        f.render_widget(
            Paragraph::new(Span::styled(
                "x",
                Style::default().fg(active_theme.text_normal),
            )),
            App::tabbed_overlay_close_area(ssh_area),
        );
        let ssh_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(2)])
            .split(ssh_inner);
        f.render_widget(Paragraph::new(lines), ssh_chunks[0]);
        let ssh_footer_entries: &[(&'static str, &'static str)] = &[
            ("↑↓", "navigate"),
            ("Enter/→", "open or mount"),
            ("s", "ssh shell"),
            ("u/Delete", "unmount"),
            ("Tab", "switch tabs"),
            ("Esc", "close"),
        ];
        f.render_widget(
            Paragraph::new(ui::panels::shortcut_footer_lines(
                ssh_footer_entries,
                app.active_theme,
                app.nerd_font_active,
            )),
            ssh_chunks[1],
        );
        app.footer_shortcut_zones.extend(ui::panels::footer_shortcut_zones(
            ssh_footer_entries,
            ssh_chunks[1],
            app.nerd_font_active,
        ));
}
