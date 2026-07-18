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
            app.help.scroll_offset,
            app.nerd_font_active,
            &app.keymap,
            &mut app.footer_shortcut_zones,
        );
        app.help.max_offset = max_off;
        app.help.scroll_offset = clamped_off;
        app.help.logo_native_area = logo_area;
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
            .transfer.download_pending_name
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
        if !bookmarks.is_empty() && app.bookmarks.selected >= bookmarks.len() {
            app.bookmarks.selected = bookmarks.len() - 1;
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
            app.bookmarks.selected,
            &mut app.footer_shortcut_zones,
        );
        if app.mode == AppMode::BookmarkEditing {
            let area = f.size();
            let rename_area = Rect::new(area.width / 4, area.height / 2 - 1, area.width / 2, 3);
            f.render_widget(Clear, rename_area);
            let title = format!(" Set Bookmark {} (Enter=Save, Esc=Cancel) ", app.bookmarks.edit_idx);
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
            let bm_idx = app.bookmarks.delete_idx;
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
                    button_focus: app.confirm_delete.bookmark_button_focus,
                    nerd_font_active: app.nerd_font_active,
                    theme: &active_theme,
                },
            );
        }
    } else if app.mode == AppMode::Integrations {
        let area = f.size();
        if !app.integration.rows_cache.is_empty()
            && app.integration.selected >= app.integration.rows_cache.len()
        {
            app.integration.selected = app.integration.rows_cache.len() - 1;
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
                integrations: &app.integration.rows_cache,
                integration_selected: app.integration.selected,
                search_active: app.integration.search_active,
                search_query: &app.integration.search_query,
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
                selected: app.themes.selected,
                nerd_focus: app.themes.nerd_selected,
                color_mode: app.filename_color_mode,
                color_focus: app.themes.color_selected,
                disable_clock: app.disable_clock,
                clock_focus: app.themes.clock_selected,
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
        let provider = crate::app_ai::provider_by_key(&app.ai.provider);
        let model_is_default = app.ai.model.trim().is_empty();
        let model_display = app.resolve_ai_model();
        let model_value = if model_is_default {
            format!("{} (default)", model_display)
        } else {
            model_display
        };
        let key_value = if !app.ai.api_key.trim().is_empty() {
            "•".repeat(app.ai.api_key.chars().count().min(24))
        } else if std::env::var(provider.env_var).map(|v| !v.trim().is_empty()).unwrap_or(false) {
            format!("(from ${})", provider.env_var)
        } else {
            "(not set)".to_string()
        };
        let key_is_fallback = app.ai.api_key.trim().is_empty();
        // Only show a validation glyph once the user has actually set a key.
        let key_status = if key_is_fallback {
            ui::panels::SettingsRowStatus::None
        } else {
            match app.ai.key_status {
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
            ui::panels::SettingsRow { label: "Auto AI commit message", value: if app.ai.auto_commit { "On" } else { "Off" }, dim_value: !app.ai.auto_commit, status: SettingsRowStatus::None },
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
            app.shortcuts_panel.selected,
            app.shortcuts_panel.capture,
            &mut app.footer_shortcut_zones,
        );
    } else if app.mode == AppMode::Plugins {
        let rows = app.plugin_panel_rows();
        ui::panels::render_plugins_overlay(
            f,
            ui::panels::OverlayChrome {
                anchor: tab_overlay_anchor,
                panel_tab: app.panel_tab,
                theme_id: app.active_theme,
                nerd_font: app.nerd_font_active,
            },
            &rows,
            app.plugins_panel.selected,
            app.plugins_panel.key_capture,
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
        let to_delete = &app.confirm_delete.targets;
        let folder_count = to_delete.iter().filter(|t| t.is_dir).count();
        let file_count = to_delete.len() - folder_count;
        let title = ui::dialogs::confirm_delete_title(file_count, folder_count);
        let delete_state = ui::dialogs::render_confirm_delete_dialog(
            f,
            area,
            &ui::dialogs::ConfirmDeleteView {
                title: &title,
                to_delete,
                scroll_offset: app.confirm_delete.scroll_offset,
                confirm_focused: app.confirm_delete.button_focus == 0,
                show_icons: app.show_icons,
                nerd_font_active: app.nerd_font_active,
                theme: &active_theme,
            },
            |path, path_is_symlink| {
                App::icon_for_path(path, app.show_icons, app.nerd_font_active, path_is_symlink, app.active_theme)
            },
        );
        app.confirm_delete.max_offset = delete_state.max_offset;
        app.confirm_delete.scroll_offset = delete_state.clamped_offset;
    } else if app.mode == AppMode::Organize {
        let area = f.size();
        match &app.organize.plan {
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
                        scroll_offset: app.organize.scroll_offset,
                        confirm_focused: app.organize.button_focus == 0,
                        nerd_font_active: app.nerd_font_active,
                        theme: &active_theme,
                    },
                );
                app.organize.max_offset = organize_state.max_offset;
                app.organize.scroll_offset = organize_state.clamped_offset;
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

