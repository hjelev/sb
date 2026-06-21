use super::*;
use crate::ui::theme;
use crate::util::list::{cursor_down, cursor_up};
use crate::util::tui::{resume_tui, resume_tui_cleared, suspend_tui};

pub(crate) fn handle_app_key_event_body(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
    deferred_key: &mut Option<KeyEvent>,
) -> io::Result<KeyDispatchOutcome> {
    match app.mode {
        AppMode::Browsing => return handle_browsing_key(terminal, app, key, deferred_key),
        AppMode::PathEditing => match key.code {
            KeyCode::Enter | KeyCode::Tab => {
                app.apply_path_input();
            }
            KeyCode::Esc => {
                let had_filter = app.path_input_filter.take().is_some();
                if had_filter && app.refresh_entries_or_status() {
                    app.set_status("path filter cleared");
                }
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => app.input_insert_char(c),
            _ => {}
        },
        AppMode::FolderFilter => match key.code {
            KeyCode::Esc => app.clear_folder_filter(),
            // Leave the box focused but keep it visible + filter applied, so the
            // filtered list can be navigated. Up at the top row re-enters the box.
            KeyCode::Down | KeyCode::Enter => app.mode = AppMode::Browsing,
            KeyCode::Backspace => {
                app.input_backspace();
                app.apply_folder_filter_live();
            }
            KeyCode::Delete => {
                app.input_delete();
                app.apply_folder_filter_live();
            }
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => {
                app.input_insert_char(c);
                app.apply_folder_filter_live();
            }
            _ => {}
        },
        AppMode::DbPreview => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                app.mode = AppMode::Browsing;
            }
            KeyCode::Left => {
                app.switch_sqlite_preview_table(-1);
            }
            KeyCode::Right => {
                app.switch_sqlite_preview_table(1);
            }
            KeyCode::Home => {
                if !app.db_preview_tables.is_empty() {
                    app.db_preview_selected = 0;
                    app.refresh_sqlite_preview_rows();
                }
            }
            KeyCode::End => {
                if !app.db_preview_tables.is_empty() {
                    app.db_preview_selected = app.db_preview_tables.len() - 1;
                    app.refresh_sqlite_preview_rows();
                }
            }
            _ => {}
        },
        AppMode::CommandInput => match key.code {
            KeyCode::Enter => {
                let command = app.input_buffer.clone();
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.run_shell_command_and_wait_key(&command)?;
                terminal.clear()?;
            }
            KeyCode::Esc => {
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("command cancelled");
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.input_insert_char(c)
            }
            _ => {}
        },
        AppMode::DownloadInput => match key.code {
            KeyCode::Enter => {
                app.submit_download_input();
            }
            KeyCode::Esc => {
                app.download_pending_url = None;
                app.download_pending_name = None;
                app.download_resume_input = None;
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("download cancelled");
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.paste_clipboard_at_input_cursor();
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.input_insert_char(c)
            }
            _ => {}
        },
        AppMode::DownloadNaming => match key.code {
            KeyCode::Enter => {
                app.submit_download_name();
            }
            KeyCode::Esc => {
                app.download_pending_url = None;
                app.download_pending_name = None;
                app.download_resume_input = None;
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("download cancelled");
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.paste_clipboard_at_input_cursor();
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.input_insert_char(c)
            }
            _ => {}
        },
        AppMode::GitCommitMessage => match key.code {
            KeyCode::Enter => {
                let raw = app.input_buffer.clone();
                let (commit_message, amend) = App::parse_git_commit_message(&raw);
                if commit_message.is_empty() {
                    app.set_status("commit message cannot be empty");
                } else {
                    app.clear_input_edit();
                    app.mode = AppMode::Browsing;
                    app.run_git_commit_and_push(&commit_message, amend)?;
                    terminal.clear()?;
                }
            }
            KeyCode::Esc => {
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("git commit cancelled");
                terminal.clear()?;
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.input_insert_char(c)
            }
            _ => {}
        },
        AppMode::GitTagInput => match key.code {
            KeyCode::Enter => {
                let tag = app.input_buffer.trim().to_string();
                if tag.is_empty() {
                    app.set_status("tag cannot be empty");
                } else {
                    app.clear_input_edit();
                    app.mode = AppMode::Browsing;
                    app.run_git_tag_and_push(&tag)?;
                    terminal.clear()?;
                }
            }
            KeyCode::Esc => {
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("tag creation cancelled");
                terminal.clear()?;
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.input_insert_char(c)
            }
            _ => {}
        },
        AppMode::NoteEditing => match key.code {
            KeyCode::Enter => {
                app.commit_note_edit();
            }
            KeyCode::Esc => {
                app.note_edit_targets.clear();
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => app.input_insert_char(c),
            _ => {}
        },
        AppMode::InternalSearch => return handle_internal_search_key(app, key),
        AppMode::Renaming => match key.code {
            KeyCode::Enter => {
                if let Some(old_path) = app.active_selected_entry_path() {
                    let new_path = app.active_panel_dir().join(&app.input_buffer);
                    let _ = fs::rename(old_path, new_path);
                }
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.refresh_active_panel_entries_or_status();
                app.sync_inactive_panel_if_same_dir();
            }
            KeyCode::Esc => { app.clear_input_edit(); app.mode = AppMode::Browsing; }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => app.input_insert_char(c),
            _ => {}
        },
        AppMode::PasteRenaming => match key.code {
            KeyCode::Enter => {
                let new_name = app.input_buffer.trim().to_string();
                if new_name.is_empty() {
                    app.set_status("name cannot be empty");
                } else if let Some(src) = app.paste_current_src.clone() {
                    let target_dir = app
                        .paste_target_dir
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| app.current_dir.clone());
                    let dest = target_dir.join(&new_name);
                    if dest.exists() {
                        app.set_status("target still exists: choose another name");
                    } else {
                        app.paste_current_src = None;
                        app.clear_input_edit();
                        app.mode = AppMode::Browsing;
                        if app.paste_move_mode && fs::rename(&src, &dest).is_ok() {
                            app.paste_ok_items += 1;
                            let _ = app.refresh_entries();
                            app.sync_inactive_panel_if_same_dir();
                            app.advance_paste_queue();
                            return Ok(KeyDispatchOutcome::ContinueLoop);
                        }
                        app.start_copy_job(src, dest, new_name);
                    }
                } else {
                    app.mode = AppMode::Browsing;
                }
            }
            KeyCode::Esc => {
                app.paste_queue.clear();
                app.paste_current_src = None;
                app.paste_move_mode = false;
                app.paste_target_dir = None;
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("paste cancelled");
                app.refresh_entries_or_status();
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => app.input_insert_char(c),
            _ => {}
        },
        AppMode::NewFile | AppMode::NewFolder => match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    app.input_insert_char('\n');
                } else {
                    let default_is_dir = app.mode == AppMode::NewFolder;
                    app.create_entries_from_input(default_is_dir);
                }
            }

            KeyCode::Esc => {
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => app.input_insert_char(c),
            _ => {}
        },
        AppMode::ArchiveCreate => match key.code {
            KeyCode::Enter => {
                app.create_archive_from_input();
            }
            KeyCode::Esc => {
                app.archive.create_targets.clear();
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("archive creation cancelled");
            }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => app.input_insert_char(c),
            _ => {}
        },
        AppMode::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => {
                app.mode = AppMode::Browsing;
            }
            KeyCode::BackTab => {
                app.panel_tab = 6;
                app.theme_selected = theme::themes()
                    .iter()
                    .position(|theme| theme.id == app.active_theme)
                    .unwrap_or(0);
                app.theme_panel_nerd_selected = false;
                app.theme_panel_color_selected = false;
                app.theme_panel_clock_selected = false;
                app.mode = AppMode::Themes;
            }
            KeyCode::Up => {
                app.help_scroll_offset = app.help_scroll_offset.saturating_sub(1);
            }
            KeyCode::Down => {
                app.help_scroll_offset = (app.help_scroll_offset + 1).min(app.help_max_offset);
            }
            KeyCode::PageUp => {
                app.help_scroll_offset = app.help_scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                app.help_scroll_offset = (app.help_scroll_offset + 10).min(app.help_max_offset);
            }
            KeyCode::Home => {
                app.help_scroll_offset = 0;
            }
            KeyCode::End => {
                app.help_scroll_offset = app.help_max_offset;
            }
            KeyCode::Tab => {
                app.panel_tab = 1;
                app.start_internal_search();
            }
            KeyCode::Char('c') => {
                let config_path = std::env::var("XDG_CONFIG_HOME")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .or_else(|| {
                        std::env::var("HOME")
                            .ok()
                            .map(|h| std::path::PathBuf::from(h).join(".config"))
                    })
                    .unwrap_or_else(|| std::path::PathBuf::from(".config"))
                    .join("sb")
                    .join("config");
                app.mode = AppMode::Browsing;
                suspend_tui()?;
                execute!(io::stdout(), Show)?;
                let _ = Command::new(crate::util::command::editor_command())
                    .arg(&config_path)
                    .status();
                resume_tui()?;
                execute!(io::stdout(), Hide)?;
                terminal.clear()?;
            }
            _ => {}
        }
        AppMode::Integrations => {
            // While the search bar is focused, printable keys edit the filter.
            if app.integration_search_active {
                match key.code {
                    KeyCode::Esc => {
                        app.reset_integration_search();
                        app.refresh_integration_rows_cache();
                        return Ok(KeyDispatchOutcome::ContinueLoop);
                    }
                    KeyCode::Backspace => {
                        app.integration_search_query.pop();
                        app.integration_selected = 0;
                        app.refresh_integration_rows_cache();
                        return Ok(KeyDispatchOutcome::ContinueLoop);
                    }
                    // Space toggles the selected integration even while the
                    // search bar is focused; integration names never contain
                    // spaces, so this is safe to fall through to the handler below.
                    KeyCode::Char(c)
                        if c != ' '
                            && !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        app.integration_search_query.push(c);
                        app.integration_selected = 0;
                        app.refresh_integration_rows_cache();
                        return Ok(KeyDispatchOutcome::ContinueLoop);
                    }
                    _ => {}
                }
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('I') | KeyCode::Char('q') => {
                    app.reset_integration_search();
                    app.mode = AppMode::Browsing;
                }
                KeyCode::Char('/') => {
                    app.integration_search_active = true;
                }
                KeyCode::BackTab => {
                    app.reset_integration_search();
                    app.refresh_integration_rows_cache();
                    app.begin_sort_menu();
                }
                KeyCode::Up => {
                    cursor_up(&mut app.integration_selected);
                }
                KeyCode::Down => {
                    cursor_down(&mut app.integration_selected, app.integration_rows_cache.len());
                }
                KeyCode::Char(' ') => {
                    let row = app.integration_rows_cache.get(app.integration_selected).cloned();
                    if let Some(row) = row {
                        if row.key == "__all_optional__" {
                            let all_on = app.all_optional_integrations_enabled();
                            app.set_all_optional_integrations(!all_on);
                        } else {
                            let (available, partially_supported, _) =
                                App::integration_support_and_detail(&row.key);
                            if !available && !partially_supported {
                                app.set_status(format!("{} is missing and cannot be toggled", row.key));
                                app.refresh_integration_rows_cache();
                                return Ok(KeyDispatchOutcome::ContinueLoop);
                            }
                            let current = app.integration_enabled(&row.key);
                            app.set_integration_enabled(&row.key, !current);
                        }
                    }
                    app.refresh_integration_rows_cache();
                }
                KeyCode::Enter => {
                    app.begin_integration_install_prompt_for_selected();
                }
                KeyCode::Tab => {
                    app.reset_integration_search();
                    app.refresh_integration_rows_cache();
                    app.panel_tab = 6;
                    app.theme_selected = theme::themes()
                        .iter()
                        .position(|theme| theme.id == app.active_theme)
                        .unwrap_or(0);
                    app.theme_panel_nerd_selected = false;
                    app.theme_panel_color_selected = false;
                    app.theme_panel_clock_selected = false;
                    app.mode = AppMode::Themes;
                }
                _ => {}
            }
        }
        AppMode::Themes => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    app.mode = AppMode::Browsing;
                }
                KeyCode::BackTab => {
                    app.panel_tab = 5;
                    app.integration_selected = 0;
                    app.reset_integration_search();
                    app.refresh_integration_rows_cache();
                    app.mode = AppMode::Integrations;
                }
                KeyCode::Tab => {
                    app.panel_tab = 0;
                    app.help_scroll_offset = 0;
                    app.mode = AppMode::Help;
                }
                KeyCode::Up => {
                    // Focus order: Nerd Fonts → Filename colors → Disable clock → theme list.
                    if app.theme_panel_nerd_selected {
                        // Already at the top row; nothing above.
                    } else if app.theme_panel_color_selected {
                        app.theme_panel_color_selected = false;
                        app.theme_panel_nerd_selected = true;
                    } else if app.theme_panel_clock_selected {
                        app.theme_panel_clock_selected = false;
                        app.theme_panel_color_selected = true;
                    } else if app.theme_selected == 0 {
                        app.theme_panel_clock_selected = true;
                    } else {
                        app.theme_selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if app.theme_panel_nerd_selected {
                        app.theme_panel_nerd_selected = false;
                        app.theme_panel_color_selected = true;
                    } else if app.theme_panel_color_selected {
                        app.theme_panel_color_selected = false;
                        app.theme_panel_clock_selected = true;
                    } else if app.theme_panel_clock_selected {
                        app.theme_panel_clock_selected = false;
                    } else {
                        let max_idx = theme::themes().len().saturating_sub(1);
                        app.theme_selected = (app.theme_selected + 1).min(max_idx);
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if app.theme_panel_nerd_selected {
                        app.toggle_nerd_font();
                    } else if app.theme_panel_color_selected {
                        app.cycle_filename_color_mode();
                    } else if app.theme_panel_clock_selected {
                        app.toggle_disable_clock();
                    } else {
                        app.apply_selected_theme();
                    }
                }
                _ => {}
            }
        }
        AppMode::SortMenu => {
            match key.code {
                KeyCode::BackTab => {
                    app.panel_tab = 3;
                    app.refresh_remote_entries();
                    app.mode = AppMode::SshPicker;
                }
                KeyCode::Tab => {
                    app.panel_tab = 5;
                    app.integration_selected = 0;
                    app.reset_integration_search();
                    app.refresh_integration_rows_cache();
                    app.mode = AppMode::Integrations;
                }
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Left => {
                    app.mode = AppMode::Browsing;
                }
                KeyCode::Up => {
                    cursor_up(&mut app.sort_menu_selected);
                }
                KeyCode::Down => {
                    cursor_down(&mut app.sort_menu_selected, App::sort_mode_options().len());
                }
                KeyCode::Enter | KeyCode::Right => {
                    app.commit_sort_menu_choice();
                }
                _ => {}
            }
        }
        AppMode::SshPicker => return handle_ssh_picker_key(terminal, app, key),
        AppMode::Bookmarks => match key.code {
            KeyCode::Esc | KeyCode::Char('b') | KeyCode::Char('q') => { app.mode = AppMode::Browsing; }
            KeyCode::BackTab => {
                app.panel_tab = 1;
                app.start_internal_search();
            }
            KeyCode::Tab => {
                app.panel_tab = 3;
                app.refresh_remote_entries();
                app.mode = AppMode::SshPicker;
            }
            KeyCode::Up => {
                cursor_up(&mut app.bookmark_selected);
            }
            KeyCode::Down => {
                cursor_down(&mut app.bookmark_selected, App::load_bookmarks().len());
            }
            KeyCode::Enter | KeyCode::Right => {
                let idx = app.bookmark_selected;
                let bookmarks = App::load_bookmarks();
                let is_set = bookmarks.get(idx).map(|(_, p)| p.is_some()).unwrap_or(false);
                if is_set {
                    if let Some((_, Some(path))) = bookmarks.get(idx) {
                        app.try_enter_dir_on_active_panel(path.clone());
                    }
                    app.mode = AppMode::Browsing;
                } else {
                    let current = app.active_panel_dir().to_string_lossy().to_string();
                    app.bookmark_edit_idx = idx;
                    app.begin_input_edit(AppMode::BookmarkEditing, current);
                }
            }
            KeyCode::Char(c @ '0'..='9') => {
                let idx = (c as u8 - b'0') as usize;
                let bookmarks = App::load_bookmarks();
                if let Some((_, Some(path))) = bookmarks.get(idx) {
                    app.try_enter_dir_on_active_panel(path.clone());
                }
                app.mode = AppMode::Browsing;
            }
            KeyCode::Char('d') => {
                let idx = app.bookmark_selected;
                let bookmarks = App::load_bookmarks();
                if bookmarks.get(idx).map(|(_, p)| p.is_some()).unwrap_or(false) {
                    app.bookmark_delete_idx = idx;
                    app.confirm_delete_bookmark_button_focus = 0;
                    app.mode = AppMode::ConfirmDeleteBookmark;
                }
            }
            _ => {}
        },
        AppMode::BookmarkEditing => match key.code {
            KeyCode::Enter => {
                let path = app.input_buffer.trim().to_string();
                if !path.is_empty() {
                    let idx = app.bookmark_edit_idx;
                    crate::util::config::SbPersistConfig::update(|cfg| {
                        cfg.bookmarks.insert(idx as u8, path);
                    });
                }
                app.clear_input_edit();
                app.mode = AppMode::Bookmarks;
            }
            KeyCode::Esc => { app.clear_input_edit(); app.mode = AppMode::Bookmarks; }
            KeyCode::Backspace => app.input_backspace(),
            KeyCode::Delete => app.input_delete(),
            KeyCode::Left => app.input_move_left(),
            KeyCode::Right => app.input_move_right(),
            KeyCode::Home => app.input_move_home(),
            KeyCode::End => app.input_move_end(),
            KeyCode::Char(c) => app.input_insert_char(c),
            _ => {}
        },
        AppMode::ConfirmDelete => {
            app.handle_confirm_delete_key(key);
        }
        AppMode::ConfirmExtract => {
            app.handle_confirm_extract_key(key);
        }
        AppMode::ConfirmDeleteBookmark => {
            app.handle_confirm_delete_bookmark_key(key);
        }
        AppMode::ConfirmDownloadOverwrite => match key.code {
            KeyCode::Enter | KeyCode::Char('y') => {
                if let (Some(url), Some(file_name)) = (
                    app.download_pending_url.clone(),
                    app.download_pending_name.clone(),
                ) {
                    app.start_download_job(url, file_name);
                } else {
                    app.mode = AppMode::Browsing;
                    app.set_status("download cancelled");
                }
            }
            KeyCode::Esc | KeyCode::Char('n') => {
                app.cancel_download_overwrite();
            }
            _ => {}
        },
        AppMode::ConfirmIntegrationInstall => {
            if app.handle_confirm_integration_install_key(key)? {
                terminal.clear()?;
            }
        }
    }

    Ok(KeyDispatchOutcome::Ok)
}

fn handle_browsing_key(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
    deferred_key: &mut Option<KeyEvent>,
) -> io::Result<KeyDispatchOutcome> {
    match key.code {
        KeyCode::Esc if app.folder_filter_visible => {
            app.clear_folder_filter();
            return Ok(KeyDispatchOutcome::ContinueLoop);
        }
        KeyCode::Char('q') | KeyCode::Esc => return Ok(KeyDispatchOutcome::Quit),
        KeyCode::Char('/') => {
            if app.is_dual_panel_mode() {
                app.set_status("folder filter is not available in dual panel mode");
            } else {
                app.begin_folder_filter();
            }
        }
        KeyCode::Char('`') => {
            app.toggle_preview_mode();
        }
        KeyCode::Char(';') => {
            app.begin_input_edit(AppMode::CommandInput, String::new());
        }
        KeyCode::Char('h') => {
            app.help_scroll_offset = 0;
            app.panel_tab = 0;
            app.mode = AppMode::Help;
        }
        KeyCode::Char('H') => return handle_git_log_key(terminal, app),
        KeyCode::Tab => {
            if app.is_dual_panel_mode() {
                app.active_panel = match app.active_panel {
                    DualPanelSide::Left => DualPanelSide::Right,
                    DualPanelSide::Right => DualPanelSide::Left,
                };
                if app.folder_size_enabled {
                    app.refresh_current_dir_free_space();
                    app.start_current_dir_total_size_scan();
                    app.start_selected_total_size_scan();
                } else if app.disable_clock {
                    // Disk pill follows the active panel even without folder sizes.
                    app.refresh_current_dir_free_space();
                }
            } else if app.is_preview_mode() {
                app.toggle_preview_pane_focus();
            } else {
                let current = app.current_path_edit_value();
                app.begin_input_edit(AppMode::PathEditing, current);
            }
        }
        KeyCode::Char(' ') | KeyCode::Insert => {
            if app.is_dual_panel_mode() && app.active_panel == crate::DualPanelSide::Right {
                if !app.right.entries.is_empty() {
                    if app.right.marked_indices.contains(&app.right.selected_index) {
                        app.right.marked_indices.remove(&app.right.selected_index);
                    } else {
                        app.right.marked_indices.insert(app.right.selected_index);
                    }
                    app.start_selected_total_size_scan();
                    if app.right.selected_index < app.right.entries.len() - 1 {
                        app.right.selected_index += 1;
                        app.right.table_state.select(Some(app.right.selected_index));
                    }
                }
            } else if !app.entries.is_empty() {
                if app.marked_indices.contains(&app.selected_index) {
                    app.marked_indices.remove(&app.selected_index);
                } else {
                    app.marked_indices.insert(app.selected_index);
                }
                app.start_selected_total_size_scan();
                if app.selected_index < app.entries.len() - 1 {
                    app.selected_index += 1;
                    app.table_state.select(Some(app.selected_index));
                }
            }
        }
        KeyCode::Char('*') => {
            if app.is_dual_panel_mode() && app.active_panel == crate::DualPanelSide::Right {
                if !app.right.entries.is_empty() {
                    if app.right.marked_indices.len() == app.right.entries.len() {
                        app.right.marked_indices.clear();
                    } else {
                        app.right.marked_indices = (0..app.right.entries.len()).collect();
                    }
                    app.start_selected_total_size_scan();
                }
            } else if !app.entries.is_empty() {
                if app.marked_indices.len() == app.entries.len() {
                    app.marked_indices.clear();
                } else {
                    app.marked_indices = (0..app.entries.len()).collect();
                }
                app.start_selected_total_size_scan();
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.copy_full_paths_to_system_clipboard();
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.begin_note_edit();
        }
        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let _ = app.drop_to_shell();
            let _ = terminal.clear();
        }
        KeyCode::Char('c') | KeyCode::F(5) => {
            if app.is_dual_panel_mode() {
                app.begin_dual_panel_transfer(false);
            } else {
                app.clipboard.clear();
                if !app.marked_indices.is_empty() {
                    // Copy all marked
                    for &idx in &app.marked_indices {
                        if let Some(e) = app.entries.get(idx) { app.clipboard.push(e.path()); }
                    }
                } else if let Some(e) = app.entries.get(app.selected_index) {
                    // Copy single selected
                    app.clipboard.push(e.path());
                }
            }
        }
        KeyCode::Char('w') => {
            app.begin_download_input();
        }
        KeyCode::Char('v') => {
            app.begin_paste();
        }
        KeyCode::Char('m') => {
            if app.is_dual_panel_mode() {
                app.begin_dual_panel_transfer(true);
            } else {
                app.begin_move();
            }
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.edit_system_clipboard_via_temp_file()?;
            terminal.clear()?;
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            if !app.active_entries_empty() {
                app.begin_confirm_delete();
            }
        }
        KeyCode::Char('x') => {
            app.toggle_executable_permissions();
        }
        KeyCode::Char('p') => {
            if let Some(selected_path) = app.active_selected_entry_path() {
                if selected_path.is_dir() {
                    app.set_status("age protection works on files only");
                } else if !app.integration_active("age") {
                    app.status_tool_not_found("age");
                } else if App::is_age_protected_file(&selected_path) {
                    app.unprotect_file_with_age(&selected_path)?;
                    terminal.clear()?;
                } else {
                    app.protect_file_with_age(&selected_path)?;
                    terminal.clear()?;
                }
            }
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.begin_sort_menu();
        }
        KeyCode::Char('s') => {
            let enabled = !app.folder_size_enabled;
            app.set_folder_size_enabled(enabled);
        }
        KeyCode::Char('+') => {
            if app.consume_quick_tree_double_tap('+') {
                app.expand_tree_to_max_on_selected_dirs();
            } else {
                app.expand_tree_on_selected_dirs(1);
            }
        }
        KeyCode::Char('-') => {
            if app.consume_quick_tree_double_tap('-') {
                app.collapse_all_tree_expansions();
            } else {
                app.contract_tree_on_selected_dirs(1);
            }
        }
        KeyCode::Char('C') => {
            app.run_delta_compare()?;
            terminal.clear()?;
        }
        KeyCode::Char('o') => {
            app.open_selected_with_default_app()?;
            terminal.clear()?;
        }
        KeyCode::Char('t') => {
            app.open_todo_file_in_editor()?;
            terminal.clear()?;
        }
        KeyCode::Char('i') => {
            app.open_split_shell_with_less()?;
            terminal.clear()?;
        }
        KeyCode::Char('E') => {
            app.open_split_shell_with_editor()?;
            terminal.clear()?;
        }
        KeyCode::Char('l') => {
            if let Some(selected_path) = app.active_selected_entry_path()
                && !selected_path.is_dir() {
                    suspend_tui()?;
                    if App::is_binary_file(&selected_path) && app.integration_active("hexyl") {
                        let mut hexyl = Command::new("hexyl");
                        hexyl.arg(&selected_path);
                        crate::util::command::pipe_to_pager_or_less(hexyl, &selected_path);
                    } else {
                        let _ = Command::new("less")
                            .args(["-R", selected_path.to_str().unwrap_or_default()])
                            .status();
                    }
                    resume_tui()?;
                    terminal.clear()?;
                }
        }
        KeyCode::Char('n') => {
            app.begin_input_edit(AppMode::NewFile, String::new());
        }
        KeyCode::Char('Z') => {
            app.run_zip_action();
        }
        KeyCode::Char('~') => {
            if let Ok(home) = env::var("HOME") {
                let home_path = PathBuf::from(home);
                if home_path.is_dir() {
                    app.try_enter_dir_on_active_panel(home_path);
                }
            }
        }
        KeyCode::Char('b') => { app.panel_tab = 2; app.mode = AppMode::Bookmarks; }
        KeyCode::Char('I') => {
            app.integration_selected = 0;
            app.reset_integration_search();
            app.refresh_integration_rows_cache();
            app.panel_tab = 5;
            app.mode = AppMode::Integrations;
        }
        KeyCode::Char('T') => {
            app.panel_tab = 6;
            app.theme_selected = theme::themes()
                .iter()
                .position(|theme| theme.id == app.active_theme)
                .unwrap_or(0);
            app.theme_panel_nerd_selected = false;
            app.theme_panel_color_selected = false;
            app.mode = AppMode::Themes;
        }
        KeyCode::Char('S') => {
            let has_sshfs = app.integration_active("sshfs");
            let has_rclone = app.integration_active("rclone");
            app.refresh_remote_entries();
            if app.remote_entries.is_empty() {
                if !has_sshfs && !has_rclone {
                    app.set_status("No media mounts or mounted archives found (sshfs/rclone not installed)");
                } else {
                    app.set_status("No SSH/rclone/media mounts or mounted archives found");
                }
            } else {
                app.panel_tab = 3;
                app.mode = AppMode::SshPicker;
            }
        }
        KeyCode::Char(c @ '0'..='9') => {
            let idx = (c as u8 - b'0') as usize;
            if let Ok(path_str) = env::var(format!("SB_BOOKMARK_{}", idx)) {
                let path = PathBuf::from(&path_str);
                if path.is_dir() {
                    app.try_enter_dir_on_active_panel(path);
                }
            }
        }
        KeyCode::Char('.') => {
            app.show_hidden = !app.show_hidden;
            app.refresh_entries_or_status();
            app.set_status(if app.show_hidden {
                "hidden files: shown"
            } else {
                "hidden files: hidden"
            });
        }

        KeyCode::F(2) | KeyCode::Char('r') => {
            if app.marked_indices.len() > 1 {
                if !app.integration_active("vidir") {
                    app.status_tool_not_found("vidir");
                } else {
                    let targets: Vec<PathBuf> = app.entries
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| app.marked_indices.contains(i))
                        .map(|(_, e)| e.path())
                        .collect();
                    if targets.is_empty() {
                        app.set_status("no selected item to rename");
                    } else {
                        suspend_tui()?;
                        let mut cmd = Command::new("vidir");
                        for p in &targets {
                            cmd.arg(p);
                        }
                        let _ = cmd.status();
                        resume_tui()?;
                        terminal.clear()?;
                        app.refresh_entries_or_status();
                    }
                }
            } else {
                let target_idx = if app.marked_indices.len() == 1 {
                    *app.marked_indices.iter().next().unwrap_or(&app.selected_index)
                } else {
                    app.selected_index
                };
                if let Some(e) = app.entries.get(target_idx) {
                    app.selected_index = target_idx;
                    app.table_state.select(Some(target_idx));
                    let current_name = crate::util::classify::entry_name(e);
                    app.begin_input_edit(AppMode::Renaming, current_name);
                }
            }
        }
        KeyCode::Up | KeyCode::Down => return handle_vertical_move_key(app, key, deferred_key),
        KeyCode::PageUp => {
            if app.preview_focus_is_preview() {
                app.preview_scroll_up(8);
            } else if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                app.right.selected_index = app.right.selected_index.saturating_sub(app.page_size);
                app.right.table_state.select(Some(app.right.selected_index));
            } else {
                app.selected_index = app.selected_index.saturating_sub(app.page_size);
                app.table_state.select(Some(app.selected_index));
            }
        }
        KeyCode::PageDown => {
            if app.preview_focus_is_preview() {
                app.preview_scroll_down(8);
            } else if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                if !app.right.entries.is_empty() {
                    app.right.selected_index = (app.right.selected_index + app.page_size).min(app.right.entries.len() - 1);
                    app.right.table_state.select(Some(app.right.selected_index));
                }
            } else if !app.entries.is_empty() {
                app.selected_index = (app.selected_index + app.page_size).min(app.entries.len() - 1);
                app.table_state.select(Some(app.selected_index));
            }
        }
        KeyCode::Home => {
            if app.preview_focus_is_preview() {
                app.preview_scroll_offset = 0;
            } else if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                app.right.selected_index = 0;
                app.right.table_state.select(Some(0));
            } else {
                app.selected_index = 0;
                app.table_state.select(Some(0));
            }
        }
        KeyCode::End => {
            if app.preview_focus_is_preview() {
                app.preview_scroll_offset = app.preview_max_scroll();
            } else if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                if !app.right.entries.is_empty() {
                    app.right.selected_index = app.right.entries.len() - 1;
                    app.right.table_state.select(Some(app.right.selected_index));
                }
            } else if !app.entries.is_empty() {
                app.selected_index = app.entries.len() - 1;
                app.table_state.select(Some(app.selected_index));
            }
        }
        KeyCode::Left | KeyCode::Backspace => {
            if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                if !app.try_leave_archive()
                    && let Some(parent) = app.right.dir.parent() {
                        app.right.dir = parent.to_path_buf();
                        let _ = app.refresh_right_panel_entries();
                    }
                return Ok(KeyDispatchOutcome::ContinueLoop);
            }
            if !app.try_leave_archive() && !app.try_leave_ssh_mount() {
                app.try_enter_parent_dir();
            }
        }
        KeyCode::Enter | KeyCode::Right => return handle_enter_or_right(terminal, app, key),
        KeyCode::Char('g') => return handle_grep_search_key(terminal, app),
        KeyCode::Char('G') => {
            let work_dir = app.active_panel_dir();
            if !app.integration_active("git") {
                app.status_tool_not_found("git");
            } else {
                match App::get_git_info(&work_dir) {
                    Some((_, true, _)) => {
                        let confirmed = app.preview_git_diff_and_confirm_commit()?;
                        terminal.clear()?;
                        if confirmed {
                            app.begin_input_edit(AppMode::GitCommitMessage, String::new());
                            app.set_status("enter commit message (include --amend to amend+force-push)");
                        } else {
                            app.set_status("git commit cancelled");
                        }
                    }
                    Some((_, false, _)) => {
                        app.set_status("repository is clean");
                    }
                    None => {
                        app.set_status("not a git repository");
                    }
                }
            }
        }
        KeyCode::Char('f') => return handle_fzf_find_key(terminal, app),
        KeyCode::Char('e') | KeyCode::F(4) => return handle_edit_key(terminal, app),
        _ => {}
    }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_git_log_key(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<KeyDispatchOutcome> {
            let work_dir = app.active_panel_dir();
            if app.integration_active("git")
                && App::get_git_info(&work_dir).is_some()
            {
                let fmt = "%C(bold blue)%h%C(reset) - %C(cyan)%ad%C(reset) | %C(yellow)%d%C(reset) %C(white)%s%C(reset) %C(green)[%an]%C(reset)";
                suspend_tui()?;
                execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
                let mut log_cmd = Command::new("git");
                log_cmd
                    .args([
                        "log",
                        "--graph",
                        &format!("--pretty=format:{}", fmt),
                        "--date=short",
                        "--all",
                        "--color=always",
                    ])
                    .current_dir(&work_dir)
                    .stderr(Stdio::null());
                // Pipe the colored log into the pager; if that's unavailable, fall
                // back to streaming an uncolored log straight to the terminal.
                if !crate::util::command::pipe_to_pager(log_cmd) {
                    crate::util::command::CommandBuilder::git_interactive(
                        &work_dir,
                        &[
                            "log",
                            "--graph",
                            &format!("--pretty=format:{}", fmt),
                            "--date=short",
                            "--all",
                        ],
                    );
                }
                resume_tui_cleared()?;
                enable_raw_mode()?;
                terminal.clear()?;
            } else {
                app.set_status("not a git repository");
            }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_vertical_move_key(app: &mut App, key: KeyEvent, deferred_key: &mut Option<KeyEvent>) -> io::Result<KeyDispatchOutcome> {
            // With the folder-filter box open and focus on the list, pressing Up
            // at the top row returns keyboard focus to the box.
            if key.code == KeyCode::Up && app.folder_filter_visible {
                let at_top = if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                    app.right.selected_index == 0
                } else {
                    app.selected_index == 0
                };
                if at_top {
                    app.mode = AppMode::FolderFilter;
                    app.input_cursor = app.input_buffer.chars().count();
                    return Ok(KeyDispatchOutcome::ContinueLoop);
                }
            }
            if app.preview_focus_is_preview() {
                if key.code == KeyCode::Up {
                    app.preview_scroll_up(1);
                } else {
                    app.preview_scroll_down(1);
                }
                return Ok(KeyDispatchOutcome::ContinueLoop);
            }

            if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                if app.right.entries.is_empty() {
                    return Ok(KeyDispatchOutcome::ContinueLoop);
                }

                let mut steps: usize = 1;
                while steps < 32 && event::poll(Duration::from_millis(0))? {
                    match event::read()? {
                        Event::Key(next)
                            if next.code == key.code
                                && next.modifiers == key.modifiers
                                && next.kind == key.kind =>
                        {
                            steps += 1;
                        }
                        Event::Key(next) => {
                            *deferred_key = Some(next);
                            break;
                        }
                        _ => {}
                    }
                }

                let max_idx = app.right.entries.len().saturating_sub(1);
                let next_idx = if key.code == KeyCode::Up {
                    app.right.selected_index.saturating_sub(steps)
                } else {
                    (app.right.selected_index + steps).min(max_idx)
                };
                app.right.selected_index = next_idx;
                app.right.table_state.select(Some(next_idx));
                return Ok(KeyDispatchOutcome::ContinueLoop);
            }

            let mut steps: usize = 1;
            while steps < 32 && event::poll(Duration::from_millis(0))? {
                match event::read()? {
                    Event::Key(next)
                        if next.code == key.code
                            && next.modifiers == key.modifiers
                            && next.kind == key.kind =>
                    {
                        steps += 1;
                    }
                    Event::Key(next) => {
                        *deferred_key = Some(next);
                        break;
                    }
                    _ => {}
                }
            }

            let delta = if key.code == KeyCode::Up {
                -(steps as isize)
            } else {
                steps as isize
            };
            app.move_selection_delta(delta);
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_enter_or_right(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App, key: KeyEvent) -> io::Result<KeyDispatchOutcome> {
            // Right in the right panel only navigates into directories; Enter opens everything.
            if key.code == KeyCode::Right && app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                if let Some(selected_path) = app.right.entries.get(app.right.selected_index).map(|e| e.path())
                    && selected_path.is_dir() {
                        app.try_enter_dir_on_active_panel(selected_path);
                    }
                return Ok(KeyDispatchOutcome::ContinueLoop);
            }

            let selected_path = if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                app.right.entries.get(app.right.selected_index).map(|e| e.path())
            } else {
                app.entries.get(app.selected_index).map(|e| e.path())
            };

            if let Some(selected_path) = selected_path {
                if selected_path.is_dir() {
                    app.try_enter_dir_on_active_panel(selected_path);
                }
                else if App::is_age_protected_file(&selected_path) {
                    if !app.integration_active("age") {
                        app.status_tool_not_found("age");
                    } else if app.preview_age_file(&selected_path)? {
                        terminal.clear()?;
                    }
                }
                else if App::is_fuse_zip_archive(&selected_path) && app.integration_active("fuse-zip") {
                    let _ = app.try_mount_archive(selected_path);
                }
                else if App::is_archivemount_archive(&selected_path) && app.integration_active("archivemount") {
                    let _ = app.try_mount_archive_with(selected_path, "archivemount");
                }
                else if App::is_supported_archive(&selected_path) {
                    let _ = app.preview_archive_contents(&selected_path);
                    terminal.clear()?;
                }
                else if App::is_image_file(&selected_path)
                    || (App::is_svg_file(&selected_path) && app.integration_active("resvg")) {
                    let is_bitmap_image = App::is_image_file(&selected_path);
                    if app.preview_images_with_native(selected_path.clone())? {
                        terminal.clear()?;
                    } else if app.preview_images_with_halfblock_fullscreen(selected_path.clone())? {
                        terminal.clear()?;
                    } else if is_bitmap_image && app.integration_active("viu") {
                        app.preview_images_with_viu(selected_path)?;
                        terminal.clear()?;
                    } else if is_bitmap_image && app.integration_active("chafa") {
                        app.preview_images_with_chafa(selected_path)?;
                        terminal.clear()?;
                    } else {
                        app.set_status("image preview unavailable (native, halfblock, viu, chafa, resvg)");
                    }
                }
                else if App::is_markdown_file(&selected_path) && app.integration_active("glow") {
                    suspend_tui()?;
                    let _ = Command::new("glow")
                        .arg("-p")
                        .arg(&selected_path)
                        .status();
                    resume_tui()?;
                    terminal.clear()?;
                }
                else if App::is_mermaid_file(&selected_path) && app.integration_active("mmdflux") {
                    suspend_tui()?;
                    let mut cmd = Command::new("mmdflux");
                    cmd.arg(&selected_path);
                    let _ = crate::util::command::pipe_to_pager(cmd);
                    resume_tui()?;
                    terminal.clear()?;
                }
                else if App::is_html_file(&selected_path) && app.integration_active("links") {
                    suspend_tui()?;
                    let _ = Command::new("links").arg(&selected_path).status();
                    resume_tui()?;
                    terminal.clear()?;
                }
                else if App::is_json_file(&selected_path) && app.integration_active("jnv") {
                    suspend_tui()?;
                    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
                    let _ = App::preview_json_with_jnv(&selected_path);
                    resume_tui()?;
                    terminal.clear()?;
                }
                else if App::is_delimited_text_file(&selected_path) && app.integration_active("csvlens") {
                    suspend_tui()?;
                    let _ = Command::new("csvlens").arg(&selected_path).status();
                    resume_tui()?;
                    terminal.clear()?;
                }
                else if App::is_sqlite_db_file(&selected_path) {
                    if app.integration_active("sqlite3") {
                        app.begin_sqlite_preview(selected_path);
                    } else {
                        app.status_tool_not_found("sqlite3");
                    }
                }
                else if App::is_audio_file(&selected_path) && app.integration_active("sox") {
                    suspend_tui()?;
                    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;

                    let (player, extra): (&str, &[&str]) = if App::integration_probe("play").0 {
                        ("play", &[])
                    } else {
                        ("sox", &["-d"])
                    };
                    let mut child =
                        crate::util::command::spawn_detached(player, &selected_path, extra);

                    if let Ok(ref mut proc) = child {
                        println!("Playing: {}", selected_path.display());
                        println!("Press q, Esc, or Left to stop playback.");
                        enable_raw_mode()?;
                        loop {
                            if proc.try_wait()?.is_some() {
                                break;
                            }
                            if event::poll(Duration::from_millis(120))?
                                && let Event::Key(k) = event::read()?
                                    && matches!(k.code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left) {
                                        let _ = proc.kill();
                                        let _ = proc.wait();
                                        break;
                                    }
                        }
                        disable_raw_mode()?;
                    }

                    resume_tui()?;
                    terminal.clear()?;
                }
                else if App::is_pdf_file(&selected_path) && app.integration_active("pdftotext") {
                    suspend_tui()?;

                    let mut cmd = Command::new("pdftotext");
                    cmd.args(["-layout", "-nopgbrk"]).arg(&selected_path).arg("-");
                    crate::util::command::pipe_to_pager_or_less(cmd, &selected_path);

                    resume_tui()?;
                    terminal.clear()?;
                }
                else if App::is_cast_file(&selected_path) && app.integration_active("asciinema") {
                    suspend_tui()?;

                    let _ = App::preview_cast_with_asciinema(&selected_path)?;

                    resume_tui()?;
                    terminal.clear()?;
                }
                else {
                    suspend_tui()?;
                    if App::is_binary_file(&selected_path) && app.integration_active("hexyl") {
                        let mut cmd = Command::new("hexyl");
                        cmd.arg(&selected_path);
                        let _ = crate::util::command::pipe_to_pager(cmd);
                    } else if app.integration_active("bat") {
                        let bat_cmd = App::bat_tool().unwrap_or_else(|| "bat".to_string());
                        let _ = Command::new(bat_cmd)
                            .args(["--paging=always", "--style=full", "--color=always"])
                            .arg(&selected_path)
                            .status();
                    } else {
                        let _ = Command::new("less").arg("-R").arg(&selected_path).status();
                    }
                    resume_tui()?;
                    terminal.clear()?;
                }
            }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_grep_search_key(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<KeyDispatchOutcome> {
            let work_dir = app.active_panel_dir();
            let has_rg  = app.integration_active("rg");
            let has_fzf = app.integration_active("fzf");
            if has_rg {
                let tmp = App::create_temp_selection_path("sbrs_fzf_rg_selection");
                let cmd = if has_fzf {
                    // rg pipes into fzf; fzf writes its selection to temp file.
                    // Using inherited stdio so fzf owns the real TTY on all platforms.
                    format!(
                        "rg --color=always --line-number --no-heading --smart-case \
                         --fixed-strings --colors=match:fg:214 '' 2>/dev/null \
                         | fzf --ansi --exact --layout=reverse --delimiter=: \
                         | awk -F: '{{print $1}}' > {}",
                        tmp.display()
                    )
                } else {
                    // no fzf: pick first file with a match
                    format!(
                        "rg --files-with-matches '' 2>/dev/null | head -1 > {}",
                        tmp.display()
                    )
                };
                suspend_tui()?;
                let _ = Command::new("sh")
                    .args(["-c", &cmd])
                    .current_dir(&work_dir)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status();
                resume_tui()?;
                terminal.clear()?;
                let selected = fs::read_to_string(&tmp).unwrap_or_default();
                let _ = fs::remove_file(&tmp);
                let first_line = selected.lines().next().unwrap_or("").trim().to_string();
                if !first_line.is_empty() {
                    let selected_path = work_dir.join(&first_line);
                    if let Some(parent) = selected_path.parent() {
                        app.try_enter_dir_on_active_panel(parent.to_path_buf());
                        if let Some(name) = selected_path.file_name() {
                            app.select_entry_named(&name.to_string_lossy());
                        }
                    }
                }
            } else {
                app.start_internal_search_with_scope(InternalSearchScope::Content);
                app.set_status("rg not found; opened Search in content mode");
            }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_fzf_find_key(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<KeyDispatchOutcome> {
            let work_dir = app.active_panel_dir();
            if app.integration_active("fzf") {
                let tmp = App::create_temp_selection_path("sbrs_fzf_selection");
                let cmd = format!(
                    "find . -path '*/.*' -prune -o -print 2>/dev/null | fzf --layout=reverse > {}",
                    tmp.display()
                );
                suspend_tui()?;
                let _ = Command::new("sh")
                    .args(["-c", &cmd])
                    .current_dir(&work_dir)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status();
                resume_tui()?;
                terminal.clear()?;
                let selected = fs::read_to_string(&tmp).unwrap_or_default();
                let _ = fs::remove_file(&tmp);
                let selected = selected.trim().to_string();
                if !selected.is_empty() {
                    let selected_path = work_dir.join(&selected);
                    if let Some(parent) = selected_path.parent() {
                        app.try_enter_dir_on_active_panel(parent.to_path_buf());
                        if let Some(name) = selected_path.file_name() {
                            app.select_entry_named(&name.to_string_lossy());
                        }
                    }
                }
            } else {
                app.start_internal_search();
            }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_edit_key(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> io::Result<KeyDispatchOutcome> {
            if let Some(path) = app.active_selected_entry_path() {
                if path.is_dir() {
                    let current_name = crate::util::classify::path_file_name(&path).unwrap_or_default();
                    app.begin_input_edit(AppMode::Renaming, current_name);
                } else if App::is_age_protected_file(&path) {
                    if !app.integration_active("age") {
                        app.status_tool_not_found("age");
                    } else if app.edit_age_file(&path)? {
                        terminal.clear()?;
                    }
                } else {
                    suspend_tui()?;
                    execute!(io::stdout(), Show)?;
                    if !path.is_dir() && App::is_binary_file(&path) && app.integration_active("hexedit") {
                        let _ = Command::new("hexedit").arg(&path).status();
                    } else {
                        let _ = Command::new(crate::util::command::editor_command()).arg(&path).status();
                    }
                    resume_tui()?;
                    execute!(io::stdout(), Hide)?;
                    terminal.clear()?;
                    app.refresh_entries_or_status();
                }
            }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_internal_search_key(app: &mut App, key: KeyEvent) -> io::Result<KeyDispatchOutcome> {
    match key.code {
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.search.scope == InternalSearchScope::Content {
                app.search.limits_menu_open = !app.search.limits_menu_open;
            }
        }
        KeyCode::Esc if app.search.limits_menu_open => {
            app.search.limits_menu_open = false;
        }
        KeyCode::Enter if app.search.limits_menu_open => {
            app.search.limits_menu_open = false;
        }
        KeyCode::Up if app.search.limits_menu_open => {
            app.search.limits_selected = app.search.limits_selected.saturating_sub(1);
        }
        KeyCode::Down if app.search.limits_menu_open => {
            app.search.limits_selected = (app.search.limits_selected + 1).min(2);
        }
        KeyCode::Left if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(false, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Right if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(true, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('-') if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(false, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('+') if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(true, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('=') if app.search.limits_menu_open => {
            app.adjust_internal_search_content_limit(true, key.modifiers.contains(KeyModifiers::SHIFT));
        }
        KeyCode::Char('r') if app.search.limits_menu_open => {
            app.reset_internal_search_content_limits_to_defaults();
        }
        KeyCode::Backspace | KeyCode::Delete | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
            if app.search.limits_menu_open =>
        {
        }
        KeyCode::Char(_)
            if app.search.limits_menu_open
                && !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
        }
        KeyCode::Esc => {
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.clear_input_edit();
            app.mode = AppMode::Browsing;
        }
        KeyCode::BackTab => {
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.panel_tab = 0;
            app.help_scroll_offset = 0;
            app.mode = AppMode::Help;
        }
        KeyCode::Tab => {
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.panel_tab = 2;
            app.mode = AppMode::Bookmarks;
        }
        KeyCode::Enter => {
            let selected_path = app.selected_internal_search_path();
            app.cancel_internal_search_candidate_scan();
            app.cancel_internal_search_content_request();
            app.clear_input_edit();
            app.mode = AppMode::Browsing;
            if let Some(path) = selected_path
                && let Some(parent) = path.parent() {
                    app.try_enter_dir(parent.to_path_buf());
                    if let Some(name) = path.file_name() {
                        app.select_entry_named(&name.to_string_lossy());
                    }
                }
        }
        KeyCode::Up => {
            app.search.selected = app.search.selected.saturating_sub(1);
        }
        KeyCode::Down => {
            let max_idx = app.search.results.len().saturating_sub(1);
            app.search.selected = (app.search.selected + 1).min(max_idx);
        }
        KeyCode::PageUp => {
            app.search.selected = app.search.selected.saturating_sub(10);
        }
        KeyCode::PageDown => {
            let max_idx = app.search.results.len().saturating_sub(1);
            app.search.selected = (app.search.selected + 10).min(max_idx);
        }
        KeyCode::Backspace => {
            app.input_backspace();
            app.refresh_internal_search_results();
        }
        KeyCode::Delete => {
            app.input_delete();
            app.refresh_internal_search_results();
        }
        KeyCode::Left => app.input_move_left(),
        KeyCode::Right => app.input_move_right(),
        KeyCode::Home => {
            app.input_move_home();
        }
        KeyCode::End => {
            app.input_move_end();
        }
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_internal_search_scope();
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.input_insert_char(c);
            app.refresh_internal_search_results();
        }
        _ => {}
    }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_ssh_picker_key(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
) -> io::Result<KeyDispatchOutcome> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => { app.mode = AppMode::Browsing; }
        KeyCode::BackTab => {
            app.panel_tab = 2;
            app.mode = AppMode::Bookmarks;
        }
        KeyCode::Tab => {
            app.begin_sort_menu();
        }
        KeyCode::Up => {
            if app.ssh_picker_selection > 0 {
                app.ssh_picker_selection -= 1;
            }
        }
        KeyCode::Down => {
            if !app.remote_entries.is_empty() && app.ssh_picker_selection < app.remote_entries.len() - 1 {
                app.ssh_picker_selection += 1;
            }
        }
        KeyCode::Enter | KeyCode::Right => {
            if let Some(entry) = app.remote_entries.get(app.ssh_picker_selection).cloned() {
                let alias = entry.alias().to_string();
                match entry {
                    RemoteEntry::Ssh(host) => {
                        let already_mounted = app.ssh_mounts.iter().any(|m| m.host_alias == alias);
                        if already_mounted {
                            app.mount_ssh_host(&host)?;
                        } else {
                            suspend_tui()?;
                            let result = app.mount_ssh_host(&host);
                            resume_tui()?;
                            terminal.clear()?;
                            if result.is_err() {
                                app.set_status(format!("Failed to mount {}", alias));
                                app.mode = AppMode::Browsing;
                            }
                        }
                    }
                    RemoteEntry::Rclone { name, rtype } => {
                        let already_mounted = app.ssh_mounts.iter().any(|m| m.host_alias == alias);
                        if already_mounted {
                            app.mount_rclone_remote(&name, &rtype)?;
                        } else {
                            suspend_tui()?;
                            println!("Connecting to rclone remote: {}…", name);
                            let result = app.mount_rclone_remote(&name, &rtype);
                            resume_tui()?;
                            terminal.clear()?;
                            if result.is_err() {
                                app.set_status(format!("Failed to mount rclone remote {}", name));
                                app.mode = AppMode::Browsing;
                            }
                        }
                    }
                    RemoteEntry::ArchiveMount { mount_path, archive_name } => {
                        if mount_path.is_dir() {
                            app.mode = AppMode::Browsing;
                            app.try_enter_dir_on_active_panel(mount_path);
                        } else {
                            app.set_status(format!("mount not available: {}", archive_name));
                            app.mode = AppMode::Browsing;
                        }
                    }
                    RemoteEntry::LocalMount { mount_path, name, .. } => {
                        if mount_path.is_dir() {
                            app.mode = AppMode::Browsing;
                            app.try_enter_dir_on_active_panel(mount_path);
                        } else {
                            app.set_status(format!("mount not available: {}", name));
                            app.mode = AppMode::Browsing;
                        }
                    }
                }
            }
        }
        KeyCode::Char('u') | KeyCode::Delete => {
            if let Some(entry) = app.remote_entries.get(app.ssh_picker_selection).cloned() {
                match entry {
                    RemoteEntry::Ssh(host) => {
                        if app.unmount_ssh_mount_by_alias(&host.alias) {
                            app.set_status(format!("unmounted {}", host.alias));
                        } else {
                            app.set_status(format!("not mounted: {}", host.alias));
                        }
                    }
                    RemoteEntry::Rclone { name, .. } => {
                        if app.unmount_ssh_mount_by_alias(&name) {
                            app.set_status(format!("unmounted {}", name));
                        } else {
                            app.set_status(format!("not mounted: {}", name));
                        }
                    }
                    RemoteEntry::ArchiveMount { mount_path, archive_name } => {
                        if app.unmount_archive_mount_by_path(&mount_path) {
                            app.set_status(format!("unmounted {}", archive_name));
                        } else {
                            app.set_status(format!("not mounted: {}", archive_name));
                        }
                    }
                    RemoteEntry::LocalMount { name, .. } => {
                        app.set_status(format!("external mount: {} (unmount outside sb)", name));
                    }
                }

                app.refresh_remote_entries();
            }
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            if let Some(entry) = app.remote_entries.get(app.ssh_picker_selection).cloned() {
                match entry {
                    RemoteEntry::Ssh(host) => {
                        app.open_ssh_shell_session(&host)?;
                        terminal.clear()?;
                    }
                    _ => {
                        app.set_status("'s' is available only for SSH hosts");
                    }
                }
            }
        }
        _ => {}
    }
    Ok(KeyDispatchOutcome::Ok)
}

