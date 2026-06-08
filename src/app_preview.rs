use crate::{App, DualPanelSide, EntryRenderConfig, PreviewPaneFocus, ViewMode};
use crate::util::background::spawn_worker;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType};
use ratatui::prelude::*;
use crate::ui;
use crate::{PreviewBuildOptions, PreviewContentMsg, PreviewLineKind};

impl App {
    pub(crate) fn cycle_view_mode(&mut self) {
        match self.view_mode {
            ViewMode::Normal => {
                self.view_mode = ViewMode::Preview;
                self.preview_scroll_offset = 0;
                self.preview_pane_focus = PreviewPaneFocus::Folder;
                self.preview_lines = vec!["Loading preview...".to_string()];
                self.preview_footer = None;
                self.preview_image_rgb = None;
                self.preview_image_png = None;
                self.preview_native_last_key = None;
                self.request_preview_for_selected();
            }
            ViewMode::Preview => {
                self.clear_preview_state();
                self.view_mode = ViewMode::DualPanel;
                self.right.dir = self.current_dir.clone();
                self.right.selected_index = 0;
                self.right.table_state = ratatui::widgets::TableState::default();
                self.right.sort_mode = self.sort_mode;
                self.right.show_hidden = self.show_hidden;
                self.active_panel = DualPanelSide::Left;
                let _ = self.refresh_right_panel_entries();
            }
            ViewMode::DualPanel => {
                // Preserve the active panel's directory when returning to normal mode
                self.current_dir = self.active_panel_dir();
                self.view_mode = ViewMode::Normal;
                self.right.dir = std::path::PathBuf::new();
                self.right.entries.clear();
                self.right.tree_row_prefixes.clear();
                self.right.entry_render_cache.clear();
                self.right.selected_index = 0;
                self.right.marked_indices.clear();
                self.clear_selected_total_size_state_for(DualPanelSide::Right);
                self.right_status_message.clear();
                self.right.table_state = ratatui::widgets::TableState::default();
                self.active_panel = DualPanelSide::Left;
                // Refresh entries to match the new current_dir
                let _ = self.refresh_entries();
            }
        }
    }

    fn clear_preview_state(&mut self) {
        if self.preview_native_last_key.is_some() {
            match Self::terminal_image_protocol().0 {
                crate::integration::probe::TerminalImageProtocol::Kitty => {
                    let _ = Self::clear_kitty_pane_images();
                }
                crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel => {
                    if let Some(area) = self.preview_native_area {
                        let _ = Self::clear_preview_pane_area(
                            area.x,
                            area.y,
                            area.width,
                            area.height,
                        );
                    }
                }
                _ => {}
            }
        }
        self.preview_target_path = None;
        self.preview_lines.clear();
        self.preview_line_kinds.clear();
        self.preview_footer = None;
        self.preview_pending = false;
        self.preview_rx = None;
        self.preview_native_area = None;
        self.preview_native_last_key = None;
        self.preview_image_rgb = None;
        self.preview_image_png = None;
        self.preview_pane_focus = PreviewPaneFocus::Folder;
        self.preview_scroll_offset = 0;
    }

    pub(crate) fn refresh_right_panel_entries(&mut self) -> std::io::Result<()> {
        let folder_size_cache = if self.folder_size_enabled {
            Some(&self.folder_size_cache)
        } else {
            None
        };
        let entries: Vec<_> = if !self.tree_expansion_levels.is_empty() {
            let rows = crate::ui::tree::collect_tree_rows_with_expansions(
                &self.right.dir,
                self.right.show_hidden,
                self.right.sort_mode,
                folder_size_cache,
                &self.tree_expansion_levels,
            )?;
            self.right.tree_row_prefixes = rows.iter().map(|row| row.prefix.clone()).collect();
            rows.into_iter().map(|row| row.entry).collect()
        } else {
            let mut direct_entries: Vec<_> = std::fs::read_dir(&self.right.dir)?
                .filter_map(|res| res.ok())
                .filter(|e| {
                    self.right.show_hidden || !crate::util::classify::is_hidden_entry(e)
                })
                .collect();
            Self::sort_entries_by_mode(&mut direct_entries, self.right.sort_mode, folder_size_cache);
            self.right.tree_row_prefixes = vec![String::new(); direct_entries.len()];
            direct_entries
        };
        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
        };
        let uid_cache = App::build_uid_cache(&entries);
        let gid_cache = App::build_gid_cache(&entries);
        self.right.entry_render_cache = entries
            .iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        self.right.entries = entries;
        if self.folder_size_enabled {
            self.apply_cached_folder_size_columns();
            self.start_folder_size_scan();
            self.refresh_current_dir_free_space();
            self.start_current_dir_total_size_scan();
        }
        self.right.marked_indices.clear();
        self.clear_selected_total_size_state_for(DualPanelSide::Right);
        if self.right.entries.is_empty() {
            self.right.selected_index = 0;
            self.right.table_state.select(None);
        } else {
            self.right.selected_index = self.right.selected_index.min(self.right.entries.len() - 1);
            self.right.table_state.select(Some(self.right.selected_index));
        }
        self.request_notes_for_right_panel_once();
        Ok(())
    }

    pub(crate) fn toggle_preview_mode(&mut self) {
        self.cycle_view_mode();
    }

    pub(crate) fn toggle_preview_pane_focus(&mut self) {
        self.preview_pane_focus = match self.preview_pane_focus {
            PreviewPaneFocus::Folder => PreviewPaneFocus::Preview,
            PreviewPaneFocus::Preview => PreviewPaneFocus::Folder,
        };
    }

    pub(crate) fn preview_focus_is_preview(&self) -> bool {
        self.is_preview_mode() && self.preview_pane_focus == PreviewPaneFocus::Preview
    }

    pub(crate) fn preview_max_scroll(&self) -> usize {
        self.preview_lines.len().saturating_sub(1)
    }

    pub(crate) fn preview_scroll_up(&mut self, amount: usize) {
        self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(amount);
    }

    pub(crate) fn preview_scroll_down(&mut self, amount: usize) {
        let next = self.preview_scroll_offset.saturating_add(amount);
        self.preview_scroll_offset = next.min(self.preview_max_scroll());
    }

    pub(crate) fn request_preview_for_selected(&mut self) {
        if !self.is_preview_mode() {
            return;
        }
        let Some(path) = self.entries.get(self.selected_index).map(|e| e.path()) else {
            self.preview_lines = vec!["No selection".to_string()];
            self.preview_line_kinds = vec![crate::PreviewLineKind::Plain];
            self.preview_footer = None;
            self.preview_target_path = None;
            self.preview_pending = false;
            self.preview_rx = None;
            self.preview_image_rgb = None;
            self.preview_image_png = None;
            return;
        };

        if self.preview_target_path.as_ref() == Some(&path)
            && (self.preview_pending
                || !self.preview_lines.is_empty()
                || self.preview_image_rgb.is_some())
        {
            return;
        }

        self.preview_image_rgb = None;
        self.preview_image_png = None;

        if !Self::is_image_file(&path)
            && let Some(cached) = self.preview_cache.get(&path).cloned() {
                self.preview_target_path = Some(path);
                self.preview_lines = cached.0;
                self.preview_line_kinds = cached.1;
                self.preview_footer = cached.2;
                self.preview_pending = false;
                self.preview_scroll_offset = 0;
                return;
            }

        self.preview_request_id = self.preview_request_id.saturating_add(1);
        let request_id = self.preview_request_id;
        self.preview_target_path = Some(path.clone());
        self.preview_pending = true;
        self.preview_scroll_offset = 0;
        self.preview_lines = vec!["Loading preview...".to_string()];
        self.preview_line_kinds = vec![crate::PreviewLineKind::Plain];
        self.preview_footer = None;

        let opts = crate::PreviewBuildOptions {
            use_bat: Self::integration_availability_and_detail("bat").0,
            use_file: Self::integration_availability_and_detail("file").0,
            use_resvg: self.integration_active("resvg"),
            show_icons: self.show_icons,
            nerd_font_active: self.nerd_font_active,
            theme_id: self.active_theme,
        };
        self.preview_rx = Some(spawn_worker(move |tx| {
            let msg = App::build_preview_content(request_id, path, opts);
            let _ = tx.send(msg);
        }));
    }
}

impl App {
    pub(crate) fn pump_preview_progress(&mut self) {
        let Some(rx) = self.preview_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb,
            }) => {
                if request_id == self.preview_request_id {
                    self.preview_target_path = Some(path.clone());
                    if image_rgb.is_none() {
                        self.preview_cache
                            .insert(path.clone(), (lines.clone(), line_kinds.clone(), footer.clone()));
                    }
                    self.preview_lines = lines;
                    self.preview_line_kinds = line_kinds;
                    self.preview_footer = footer;
                    if let Some((ref rgb, w, h)) = image_rgb {
                        self.preview_image_png = App::encode_rgb_to_png(rgb, w, h);
                    } else {
                        self.preview_image_png = None;
                    }
                    self.preview_image_rgb = image_rgb;
                    self.preview_pending = false;
                    self.preview_scroll_offset = 0;
                }
            }
            Ok(PreviewContentMsg::Failed {
                request_id,
                path,
                message,
            }) => {
                if request_id == self.preview_request_id {
                    self.preview_target_path = Some(path);
                    self.preview_lines = vec![message];
                    self.preview_line_kinds = vec![PreviewLineKind::Plain];
                    self.preview_footer = None;
                self.preview_image_rgb = None;
                self.preview_image_png = None;
                    self.preview_pending = false;
                    self.preview_scroll_offset = 0;
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.preview_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.preview_pending = false;
            }
        }
    }

    fn build_preview_content(
        request_id: u64,
        path: PathBuf,
        opts: PreviewBuildOptions,
    ) -> PreviewContentMsg {
        let PreviewBuildOptions {
            use_bat,
            use_file,
            use_resvg,
            show_icons,
            nerd_font_active,
            theme_id,
        } = opts;
        if path.is_dir() {
            let mut entries = Vec::new();
            let mut line_kinds = Vec::new();
            let mut names = Vec::new();
            if let Ok(read_dir) = fs::read_dir(&path) {
                for item in read_dir.flatten().take(500) {
                    names.push(item.path());
                }
            }
            names.sort_by(|a, b| {
                let a_name = a
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let b_name = b
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                a_name.cmp(&b_name)
            });

            for entry_path in names {
                let file_name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| entry_path.to_string_lossy().into_owned());
                let is_symlink = entry_path
                    .symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                let is_dir = entry_path.is_dir();
                #[cfg(unix)]
                let is_executable = {
                    use std::os::unix::fs::PermissionsExt;
                    !is_dir
                        && entry_path
                            .metadata()
                            .map(|m| m.permissions().mode() & 0o111 != 0)
                            .unwrap_or(false)
                };
                #[cfg(not(unix))]
                let is_executable = false;
                let is_hidden = crate::util::classify::is_hidden_name(&file_name);

                let (icon_glyph, icon_style) = App::icon_for_name(
                    &file_name,
                    is_dir,
                    show_icons,
                    nerd_font_active,
                    is_symlink,
                    theme_id,
                );
                let icon_prefix = if show_icons && !icon_glyph.is_empty() {
                    format!(" {} ", icon_glyph)
                } else {
                    String::new()
                };
                let suffix = if is_dir { "/" } else { "" };
                entries.push(format!("{}{}{}", icon_prefix, file_name, suffix));

                let spec = ui::theme::theme_spec(theme_id);
                let mut style = if is_symlink {
                    Style::default().fg(spec.text_symlink)
                } else if is_dir {
                    Style::default()
                        .fg(spec.icon_default_dir)
                        .add_modifier(Modifier::BOLD)
                } else if is_executable {
                    Style::default().fg(spec.text_executable)
                } else {
                    icon_style.fg.map_or_else(
                        || Style::default().fg(spec.text_normal),
                        |fg| Style::default().fg(fg),
                    )
                };

                if is_hidden {
                    style = style.add_modifier(Modifier::DIM);
                }

                line_kinds.push(PreviewLineKind::Styled {
                    fg: style.fg,
                    bold: style.add_modifier.contains(Modifier::BOLD),
                    dim: style.add_modifier.contains(Modifier::DIM),
                });
            }

            if entries.is_empty() {
                entries.push("[empty folder]".to_string());
                line_kinds.push(PreviewLineKind::Plain);
            }

            let footer = App::compute_total_display_bytes(&path)
                .ok()
                .map(|bytes| format!("Total: {}", App::format_size(bytes)));
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines: entries,
                line_kinds,
                footer,
                image_rgb: None,
            };
        }

        if !path.exists() {
            return PreviewContentMsg::Failed {
                request_id,
                path,
                message: "[file not found]".to_string(),
            };
        }

        if App::is_svg_file(&path) && use_resvg {
            let image_rgb = App::decode_svg_to_rgb_scaled(&path);
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let lines = if image_rgb.is_none() {
                vec!["[svg could not be rendered]".to_string()]
            } else {
                Vec::new()
            };
            let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb,
            };
        }

        if App::is_image_file(&path) {
            let image_rgb = App::decode_image_to_rgb_scaled(&path);
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let lines = if image_rgb.is_none() {
                vec!["[image could not be decoded]".to_string()]
            } else {
                Vec::new()
            };
            let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb,
            };
        }

        let mut lines: Vec<String> = Vec::new();

        if App::is_binary_file(&path) {
            lines.push("[binary file]".to_string());
            if use_file
                && let Ok(out) = Command::new("file").arg("-b").arg(&path).output() {
                    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !text.is_empty() {
                        lines.push(text);
                    }
                }
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb: None,
            };
        }

        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > 10 * 1024 * 1024 {
            lines.push("[preview truncated: file larger than 10MB]".to_string());
        }

        // Number of leading header lines (e.g. truncation notice) that should
        // not receive a line-number gutter.
        let header_len = lines.len();

        if use_bat
            && let Ok(out) = Command::new("bat")
                .args(["--paging=never", "--style=numbers", "--color=always", "--line-range", "1:220"])
                .arg(&path)
                .output()
                && out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).into_owned();
                    lines.extend(text.lines().take(220).map(|s| s.to_string()));
                }

        // Fall back to reading the file directly when bat is unavailable or
        // produced no content, prepending our own line-number gutter.
        if lines.len() == header_len {
            let mut bytes = Vec::new();
            if let Ok(mut file) = fs::File::open(&path) {
                let _ = file.read_to_end(&mut bytes);
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            lines.extend(
                text.lines()
                    .take(220)
                    .enumerate()
                    .map(|(idx, line)| format!("\x1b[38;5;240m{:>4} │\x1b[0m {}", idx + 1, line)),
            );
        }

        if lines.is_empty() {
            lines.push("[no preview output]".to_string());
        }

        let footer = Some(format!("Size: {}", App::format_size(size)));
        let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
        PreviewContentMsg::Ready {
            request_id,
            path,
            lines,
            line_kinds,
            footer,
            image_rgb: None,
        }
    }

    pub(crate) fn preview_json_with_jnv(path: &PathBuf) -> io::Result<bool> {
        let mut child = Command::new("jnv").arg(path).spawn();
        if let Ok(ref mut proc) = child {
            println!("Viewing JSON: {}", path.display());
            println!("Press q, Esc, or Left to close preview.");
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
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn preview_single_image_with_tool(path: &PathBuf, tool: &str) -> bool {
        let script = r#"
tool="$1"
img="$2"
clear
"$tool" -- "$img"
printf '\n[Press any key to return]\n'
IFS= read -rsn1 _
"#;

        Command::new("bash")
            .arg("-lc")
            .arg(script)
            .arg("--")
            .arg(tool)
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub(crate) fn preview_cast_with_asciinema(path: &PathBuf) -> io::Result<bool> {
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;

        let mut child = match Command::new("asciinema")
            .arg("play")
            .arg(path)
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        println!("Playing cast: {}", path.display());
        println!("Press q or Esc to stop playback.");

        enable_raw_mode()?;
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if event::poll(Duration::from_millis(120))?
                && let Event::Key(k) = event::read()?
                    && matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
        }
        disable_raw_mode()?;
        Ok(true)
    }
}
