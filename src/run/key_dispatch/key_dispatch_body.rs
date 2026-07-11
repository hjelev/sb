use super::*;
use crate::ui::theme;
use crate::util::list::{cursor_down, cursor_up};
use crate::util::tui::{resume_tui, suspend_tui};

/// Shared editing keys for every single-line text-input mode: cursor movement,
/// deletion, and plain character insertion. Ctrl/Alt chords are not consumed,
/// leaving them to each mode's specific bindings (Ctrl+G, Ctrl+V, ...).
/// Returns true if the key was consumed.
fn handle_text_input_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
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
        _ => return false,
    }
    true
}

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
                let had_filter = app.left.folder_filter.take().is_some();
                if had_filter && app.refresh_entries_or_status() {
                    app.set_status("path filter cleared");
                }
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
            }
            _ => {
                handle_text_input_key(app, key);
            }
        },
        AppMode::FolderFilter => match key.code {
            KeyCode::Esc => app.clear_folder_filter(),
            // Leave the box focused but keep it visible + filter applied, so the
            // filtered list can be navigated. Up at the top row re-enters the box.
            KeyCode::Down | KeyCode::Enter => app.mode = AppMode::Browsing,
            _ => {
                let edited = matches!(
                    key.code,
                    KeyCode::Backspace | KeyCode::Delete | KeyCode::Char(_)
                );
                if handle_text_input_key(app, key) && edited {
                    app.apply_folder_filter_live();
                }
            }
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
                // Input starting with ':' runs a plugin's entry() instead of
                // a shell command (e.g. `:touch-notify`).
                if let Some(name) = command.trim().strip_prefix(':') {
                    let name = name.trim().to_string();
                    return run_plugin_entry(terminal, app, &name);
                }
                app.run_shell_command_and_wait_key(&command)?;
                terminal.clear()?;
            }
            KeyCode::Esc => {
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.set_status("command cancelled");
            }
            _ => {
                handle_text_input_key(app, key);
            }
        },
        AppMode::DownloadInput | AppMode::DownloadNaming => match key.code {
            KeyCode::Enter => {
                if app.mode == AppMode::DownloadInput {
                    app.submit_download_input();
                } else {
                    app.submit_download_name();
                }
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
            _ => {
                handle_text_input_key(app, key);
            }
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
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.request_commit_message();
            }
            _ => {
                handle_text_input_key(app, key);
            }
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
            _ => {
                handle_text_input_key(app, key);
            }
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
            _ => {
                handle_text_input_key(app, key);
            }
        },
        AppMode::InternalSearch => return handle_internal_search_key(app, key),
        AppMode::Renaming => match key.code {
            KeyCode::Enter => {
                if let Some(old_path) = app.active_selected_entry_path() {
                    let new_path = app.active_panel_dir().join(&app.input_buffer);
                    if let Err(e) = fs::rename(&old_path, &new_path) {
                        app.set_status(format!("rename failed: {}", e));
                    }
                }
                app.clear_input_edit();
                app.mode = AppMode::Browsing;
                app.refresh_active_panel_entries_or_status();
                app.sync_inactive_panel_if_same_dir();
            }
            KeyCode::Esc => { app.clear_input_edit(); app.mode = AppMode::Browsing; }
            _ => {
                handle_text_input_key(app, key);
            }
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
                        .unwrap_or_else(|| app.left.dir.clone());
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
            _ => {
                handle_text_input_key(app, key);
            }
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
            _ => {
                handle_text_input_key(app, key);
            }
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
            _ => {
                handle_text_input_key(app, key);
            }
        },
        AppMode::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => {
                app.mode = AppMode::Browsing;
            }
            KeyCode::BackTab => {
                app.open_plugins_panel();
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
                    app.panel_tab = 7;
                    app.settings_selected = 0;
                    app.mode = AppMode::Settings;
                    app.maybe_check_api_key();
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
        AppMode::Settings => match key.code {
            KeyCode::Esc => {
                app.maybe_check_api_key();
                app.mode = AppMode::Browsing;
            }
            KeyCode::BackTab => {
                app.maybe_check_api_key();
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
            KeyCode::Tab => {
                app.maybe_check_api_key();
                app.panel_tab = 8;
                app.shortcuts_selected = 0;
                app.shortcut_capture = false;
                app.mode = AppMode::Shortcuts;
            }
            KeyCode::Up => {
                let was_key = app.settings_selected == 2;
                cursor_up(&mut app.settings_selected);
                if was_key && app.settings_selected != 2 {
                    app.maybe_check_api_key();
                }
            }
            KeyCode::Down => {
                let was_key = app.settings_selected == 2;
                cursor_down(&mut app.settings_selected, 4);
                if was_key && app.settings_selected != 2 {
                    app.maybe_check_api_key();
                }
            }
            KeyCode::Left => {
                if app.settings_selected == 0 {
                    app.settings_cycle_provider(false);
                }
            }
            KeyCode::Right => {
                if app.settings_selected == 0 {
                    app.settings_cycle_provider(true);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') if app.settings_selected == 0 => {
                app.settings_cycle_provider(true);
            }
            KeyCode::Enter | KeyCode::Char(' ') if app.settings_selected == 3 => {
                app.settings_toggle_auto_commit();
            }
            KeyCode::Backspace => app.settings_input_backspace(),
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.settings_input_char(c)
            }
            _ => {}
        },
        AppMode::Shortcuts => {
            // While capturing, every key is a rebind attempt except Esc.
            if app.shortcut_capture {
                match key.code {
                    KeyCode::Esc => {
                        app.shortcut_capture = false;
                        app.set_status("rebind cancelled");
                    }
                    _ => app.apply_shortcut_capture(key),
                }
            } else {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.mode = AppMode::Browsing;
                    }
                    KeyCode::BackTab => {
                        app.panel_tab = 7;
                        app.settings_selected = 0;
                        app.mode = AppMode::Settings;
                        app.maybe_check_api_key();
                    }
                    KeyCode::Tab => {
                        app.open_plugins_panel();
                    }
                    KeyCode::Up => {
                        cursor_up(&mut app.shortcuts_selected);
                    }
                    KeyCode::Down => {
                        cursor_down(&mut app.shortcuts_selected, crate::util::keymap::ACTIONS.len());
                    }
                    KeyCode::PageUp => {
                        app.shortcuts_selected = app.shortcuts_selected.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        let max = crate::util::keymap::ACTIONS.len().saturating_sub(1);
                        app.shortcuts_selected = (app.shortcuts_selected + 10).min(max);
                    }
                    KeyCode::Home => {
                        app.shortcuts_selected = 0;
                    }
                    KeyCode::End => {
                        app.shortcuts_selected = crate::util::keymap::ACTIONS.len().saturating_sub(1);
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        app.shortcut_capture = true;
                    }
                    KeyCode::Backspace | KeyCode::Delete => {
                        app.reset_selected_shortcut();
                    }
                    _ => {}
                }
            }
        }
        AppMode::Plugins => {
            // While capturing, every key is a bind attempt except Esc.
            if app.plugin_key_capture {
                match key.code {
                    KeyCode::Esc => {
                        app.plugin_key_capture = false;
                        app.set_status("bind cancelled");
                    }
                    _ => app.apply_plugin_key_capture(key),
                }
            } else {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.mode = AppMode::Browsing;
                    }
                    KeyCode::BackTab => {
                        app.panel_tab = 8;
                        app.shortcuts_selected = 0;
                        app.shortcut_capture = false;
                        app.mode = AppMode::Shortcuts;
                    }
                    KeyCode::Tab => {
                        app.panel_tab = 0;
                        app.help_scroll_offset = 0;
                        app.mode = AppMode::Help;
                    }
                    KeyCode::Up => {
                        cursor_up(&mut app.plugins_selected);
                    }
                    KeyCode::Down => {
                        cursor_down(&mut app.plugins_selected, app.plugins.plugins.len());
                    }
                    KeyCode::Home => {
                        app.plugins_selected = 0;
                    }
                    KeyCode::End => {
                        app.plugins_selected = app.plugins.plugins.len().saturating_sub(1);
                    }
                    KeyCode::Enter => {
                        if let Some(name) = app.selected_plugin_name() {
                            app.mode = AppMode::Browsing;
                            return run_plugin_entry(terminal, app, &name);
                        }
                    }
                    KeyCode::Char(' ') => {
                        app.toggle_selected_plugin();
                    }
                    KeyCode::Char('b') => {
                        if app.selected_plugin_name().is_some() {
                            app.plugin_key_capture = true;
                        }
                    }
                    KeyCode::Backspace | KeyCode::Delete => {
                        app.reset_selected_plugin_key();
                    }
                    _ => {}
                }
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
                let len = app.bookmarks().len();
                cursor_down(&mut app.bookmark_selected, len);
            }
            KeyCode::Enter | KeyCode::Right => {
                let idx = app.bookmark_selected;
                let target = app.bookmarks().get(idx).and_then(|(_, p)| p.clone());
                if let Some(path) = target {
                    app.try_enter_dir_on_active_panel(path);
                    app.mode = AppMode::Browsing;
                } else {
                    let current = app.active_panel_dir().to_string_lossy().to_string();
                    app.bookmark_edit_idx = idx;
                    app.begin_input_edit(AppMode::BookmarkEditing, current);
                }
            }
            KeyCode::Char(c @ '0'..='9') => {
                let idx = (c as u8 - b'0') as usize;
                let target = app.bookmarks().get(idx).and_then(|(_, p)| p.clone());
                if let Some(path) = target {
                    app.try_enter_dir_on_active_panel(path);
                }
                app.mode = AppMode::Browsing;
            }
            KeyCode::Char('d') => {
                let idx = app.bookmark_selected;
                if app.bookmarks().get(idx).map(|(_, p)| p.is_some()).unwrap_or(false) {
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
                    let result = crate::util::config::SbPersistConfig::update(|cfg| {
                        cfg.bookmarks.insert(idx as u8, path);
                    });
                    if let Err(e) = result {
                        app.set_status(format!("failed to save bookmarks: {}", e));
                    }
                    app.refresh_bookmarks_cache();
                }
                app.clear_input_edit();
                app.mode = AppMode::Bookmarks;
            }
            KeyCode::Esc => { app.clear_input_edit(); app.mode = AppMode::Bookmarks; }
            _ => {
                handle_text_input_key(app, key);
            }
        },
        AppMode::ConfirmDelete => {
            app.handle_confirm_delete_key(key);
        }
        AppMode::Organize => {
            app.handle_organize_key(key);
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

