use super::*;
use crate::ui::theme;
use crate::util::tui::{resume_tui, resume_tui_cleared, suspend_tui};

pub(crate) fn handle_browsing_key(
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
                if app.size.folder_size_enabled {
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
            } else if !app.left.entries.is_empty() {
                if app.left.marked_indices.contains(&app.left.selected_index) {
                    app.left.marked_indices.remove(&app.left.selected_index);
                } else {
                    app.left.marked_indices.insert(app.left.selected_index);
                }
                app.start_selected_total_size_scan();
                if app.left.selected_index < app.left.entries.len() - 1 {
                    app.left.selected_index += 1;
                    app.left.table_state.select(Some(app.left.selected_index));
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
            } else if !app.left.entries.is_empty() {
                if app.left.marked_indices.len() == app.left.entries.len() {
                    app.left.marked_indices.clear();
                } else {
                    app.left.marked_indices = (0..app.left.entries.len()).collect();
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
                if !app.left.marked_indices.is_empty() {
                    // Copy all marked
                    for &idx in &app.left.marked_indices {
                        if let Some(e) = app.left.entries.get(idx) { app.clipboard.push(e.path()); }
                    }
                } else if let Some(e) = app.left.entries.get(app.left.selected_index) {
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
            let enabled = !app.size.folder_size_enabled;
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
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return handle_organize_workflow(app);
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
            app.left.show_hidden = !app.left.show_hidden;
            app.refresh_entries_or_status();
            app.set_status(if app.left.show_hidden {
                "hidden files: shown"
            } else {
                "hidden files: hidden"
            });
        }

        KeyCode::F(2) | KeyCode::Char('r') => {
            if app.left.marked_indices.len() > 1 {
                if !app.integration_active("vidir") {
                    app.status_tool_not_found("vidir");
                } else {
                    let targets: Vec<PathBuf> = app.left.entries
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| app.left.marked_indices.contains(i))
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
                let target_idx = if app.left.marked_indices.len() == 1 {
                    *app.left.marked_indices.iter().next().unwrap_or(&app.left.selected_index)
                } else {
                    app.left.selected_index
                };
                if let Some(e) = app.left.entries.get(target_idx) {
                    app.left.selected_index = target_idx;
                    app.left.table_state.select(Some(target_idx));
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
                app.left.selected_index = app.left.selected_index.saturating_sub(app.page_size);
                app.left.table_state.select(Some(app.left.selected_index));
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
            } else if !app.left.entries.is_empty() {
                app.left.selected_index = (app.left.selected_index + app.page_size).min(app.left.entries.len() - 1);
                app.left.table_state.select(Some(app.left.selected_index));
            }
        }
        KeyCode::Home => {
            if app.preview_focus_is_preview() {
                app.preview_scroll_offset = 0;
            } else if app.is_dual_panel_mode() && app.active_panel == DualPanelSide::Right {
                app.right.selected_index = 0;
                app.right.table_state.select(Some(0));
            } else {
                app.left.selected_index = 0;
                app.left.table_state.select(Some(0));
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
            } else if !app.left.entries.is_empty() {
                app.left.selected_index = app.left.entries.len() - 1;
                app.left.table_state.select(Some(app.left.selected_index));
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
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return handle_git_commit_workflow(terminal, app);
        }
        KeyCode::Char('g') => return handle_grep_search_key(terminal, app),
        KeyCode::Char('G') => return handle_git_commit_workflow(terminal, app),
        KeyCode::Char('f') => return handle_fzf_find_key(terminal, app),
        KeyCode::Char('e') | KeyCode::F(4) => return handle_edit_key(terminal, app),
        _ => {}
    }
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_git_commit_workflow(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> io::Result<KeyDispatchOutcome> {
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
                    if app.ai_auto_commit {
                        app.request_commit_message();
                    }
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
    Ok(KeyDispatchOutcome::Ok)
}

fn handle_organize_workflow(app: &mut App) -> io::Result<KeyDispatchOutcome> {
    if app.organize_rx.is_some() {
        app.set_status("organize plan already generating...");
        return Ok(KeyDispatchOutcome::Ok);
    }
    let work_dir = app.active_panel_dir();
    app.mode = AppMode::Organize;
    app.request_organize_plan(work_dir);
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
                    app.left.selected_index == 0
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
                app.left.entries.get(app.left.selected_index).map(|e| e.path())
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
                    if app.preview_images_with_native(selected_path.clone())?
                        || app.preview_images_with_halfblock_fullscreen(selected_path.clone())? {
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

