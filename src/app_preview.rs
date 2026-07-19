use crate::{App, DualPanelSide, EntryRenderConfig, PreviewPaneFocus, ViewMode};
use crate::util::background::{pump_once, spawn_worker};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType};
use ratatui::prelude::*;
use crate::ui;
use crate::{FilenameColorMode, PreviewBuildOptions, PreviewContentMsg, PreviewIconSpan, PreviewLineKind};

impl App {
    pub(crate) fn cycle_view_mode(&mut self) {
        match self.view_mode {
            ViewMode::Normal => {
                self.view_mode = ViewMode::Preview;
                self.preview.scroll_offset = 0;
                self.preview.pane_focus = PreviewPaneFocus::Folder;
                self.preview.lines = vec!["Loading preview...".to_string()];
                self.preview.footer = None;
                self.preview.image_rgb = None;
                self.preview.image_png = None;
                self.preview.native_last_key = None;
                self.request_preview_for_selected();
            }
            ViewMode::Preview => {
                self.clear_preview_state();
                // The folder filter (/) is not available in dual panel mode, so
                // dismiss any open filter box and drop its filter on the left panel.
                if self.folder_filter_visible {
                    self.folder_filter_visible = false;
                    if self.mode == crate::AppMode::FolderFilter {
                        self.mode = crate::AppMode::Browsing;
                    }
                    self.clear_input_edit();
                    if self.left.folder_filter.take().is_some() {
                        let _ = self.refresh_entries();
                    }
                }
                self.view_mode = ViewMode::DualPanel;
                self.right.dir = self.left.dir.clone();
                self.right.selected_index = 0;
                self.right.table_state = ratatui::widgets::TableState::default();
                self.right.sort_mode = self.left.sort_mode;
                self.right.show_hidden = self.left.show_hidden;
                self.active_panel = DualPanelSide::Left;
                let _ = self.refresh_right_panel_entries();
            }
            ViewMode::DualPanel => {
                // Preserve the active panel's directory when returning to normal mode
                self.left.dir = self.active_panel_dir();
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
        self.discard_pane_video();
        if self.preview.native_last_key.is_some() {
            match Self::terminal_image_protocol().0 {
                crate::integration::probe::TerminalImageProtocol::Kitty => {
                    let _ = Self::clear_kitty_pane_image(crate::app_images::KITTY_IMAGE_ID_PREVIEW);
                }
                crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel => {
                    if let Some(area) = self.preview.native_area {
                        let _ = Self::clear_preview_pane_area(area);
                    }
                }
                _ => {}
            }
        }
        self.preview.target_path = None;
        self.preview.lines.clear();
        self.preview.line_kinds.clear();
        self.preview.footer = None;
        self.preview.pending = false;
        self.preview.rx = None;
        self.preview.native_area = None;
        self.preview.native_last_key = None;
        self.preview.image_rgb = None;
        self.preview.image_png = None;
        self.preview.pane_focus = PreviewPaneFocus::Folder;
        self.preview.scroll_offset = 0;
    }

    pub(crate) fn refresh_right_panel_entries(&mut self) -> std::io::Result<()> {
        let folder_size_cache = if self.size.folder_size_enabled {
            Some(&self.size.folder_size_cache)
        } else {
            None
        };
        let entries: Vec<_> = if !self.tree.expansion_levels.is_empty() {
            let rows = crate::ui::tree::collect_tree_rows_with_expansions(
                &self.right.dir,
                self.right.show_hidden,
                self.right.sort_mode,
                folder_size_cache,
                &self.tree.expansion_levels,
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
        let mut entries = entries;
        if let Some(filter) = self.right.folder_filter.as_ref() {
            let filter_regex = Self::build_path_filter_regex(filter)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            let keep: Vec<bool> = entries
                .iter()
                .map(|entry| {
                    let name = crate::util::classify::entry_name(entry);
                    Self::entry_name_matches_path_filter(&name, &filter_regex)
                })
                .collect();
            let mut iter = keep.iter();
            entries.retain(|_| *iter.next().unwrap_or(&true));
            if self.right.tree_row_prefixes.len() == keep.len() {
                let mut iter = keep.iter();
                self.right
                    .tree_row_prefixes
                    .retain(|_| *iter.next().unwrap_or(&true));
            }
        }
        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
            filename_color_mode: self.filename_color_mode,
        };
        let uid_cache = App::build_uid_cache(&entries);
        let gid_cache = App::build_gid_cache(&entries);
        self.right.entry_render_cache = entries
            .iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        self.right.entries = entries;
        self.recompute_list_aggregates();
        if self.size.folder_size_enabled {
            self.apply_cached_folder_size_columns();
            self.start_folder_size_scan();
            self.refresh_current_dir_free_space();
            self.start_current_dir_total_size_scan();
        } else if self.disable_clock {
            // Disk pill follows the active panel even without folder sizes.
            self.refresh_current_dir_free_space();
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
        self.preview.pane_focus = match self.preview.pane_focus {
            PreviewPaneFocus::Folder => PreviewPaneFocus::Preview,
            PreviewPaneFocus::Preview => PreviewPaneFocus::Folder,
        };
    }

    pub(crate) fn preview_focus_is_preview(&self) -> bool {
        self.is_preview_mode() && self.preview.pane_focus == PreviewPaneFocus::Preview
    }

    pub(crate) fn preview_max_scroll(&self) -> usize {
        self.preview.lines.len().saturating_sub(1)
    }

    pub(crate) fn preview_scroll_up(&mut self, amount: usize) {
        self.preview.scroll_offset = self.preview.scroll_offset.saturating_sub(amount);
    }

    pub(crate) fn preview_scroll_down(&mut self, amount: usize) {
        let next = self.preview.scroll_offset.saturating_add(amount);
        self.preview.scroll_offset = next.min(self.preview_max_scroll());
    }

    pub(crate) fn request_preview_for_selected(&mut self) {
        if !self.is_preview_mode() {
            return;
        }
        let Some(path) = self.left.entries.get(self.left.selected_index).map(|e| e.path()) else {
            self.preview.lines = vec!["No selection".to_string()];
            self.preview.line_kinds = vec![crate::PreviewLineKind::Plain];
            self.preview.footer = None;
            self.preview.target_path = None;
            self.preview.pending = false;
            self.preview.rx = None;
            self.preview.image_rgb = None;
            self.preview.image_png = None;
            return;
        };

        if self.preview.target_path.as_ref() == Some(&path)
            && (self.preview.pending
                || !self.preview.lines.is_empty()
                || self.preview.image_rgb.is_some())
        {
            return;
        }

        self.preview.image_rgb = None;
        self.preview.image_png = None;

        // Image and video previews are never cached: their rendering depends
        // on the pane size (timg / in-process scaling), not just the path.
        if !Self::is_image_file(&path)
            && !Self::is_video_file(&path)
            && let Some(cached) = self.preview.cache.get(&path).cloned() {
                self.preview.target_path = Some(path);
                self.preview.lines = cached.0;
                self.preview.line_kinds = cached.1;
                self.preview.footer = cached.2;
                self.preview.pending = false;
                self.preview.scroll_offset = 0;
                return;
            }

        self.preview.request_id = self.preview.request_id.saturating_add(1);
        let request_id = self.preview.request_id;
        self.preview.target_path = Some(path.clone());
        self.preview.pending = true;
        self.preview.scroll_offset = 0;
        self.preview.lines = vec!["Loading preview...".to_string()];
        self.preview.line_kinds = vec![crate::PreviewLineKind::Plain];
        self.preview.footer = None;

        // Approximate the preview pane's inner cell size for tools that render
        // to a fixed geometry (timg); falls back to the 67% split of the
        // terminal when the pane layout is unavailable (e.g. overlay active).
        let (pane_cols, pane_rows) = {
            let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
            let area = ratatui::layout::Rect::new(0, 0, cols, rows);
            match self.preview_pane_frame_areas(area) {
                Some((_, preview)) => (
                    preview.width.saturating_sub(2).max(10),
                    preview.height.saturating_sub(2).max(5),
                ),
                None => (
                    (cols.saturating_mul(2) / 3).max(10),
                    rows.saturating_sub(4).max(5),
                ),
            }
        };
        let opts = crate::PreviewBuildOptions {
            use_bat: Self::integration_availability_and_detail("bat").0,
            use_file: Self::integration_availability_and_detail("file").0,
            use_resvg: self.integration_active("resvg"),
            use_timg: self.integration_active("timg"),
            use_pdftotext: self.integration_active("pdftotext"),
            use_glow: self.integration_active("glow"),
            use_doxx: self.integration_active("doxx"),
            use_xleak: self.integration_active("xleak"),
            use_sqlite3: self.integration_active("sqlite3"),
            use_sox: self.integration_active("sox"),
            use_mmdflux: self.integration_active("mmdflux"),
            use_links: self.integration_active("links"),
            use_hexyl: self.integration_active("hexyl"),
            use_zip_list: self.integration_active("zip"),
            use_tar_list: self.integration_active("tar"),
            use_7z_list: self.integration_active("7z"),
            use_rar_list: self.integration_active("rar"),
            pane_cols,
            pane_rows,
            show_icons: self.show_icons,
            nerd_font_active: self.nerd_font_active,
            theme_id: self.active_theme,
            filename_color_mode: self.filename_color_mode,
        };
        // Plugin previewer registrations are plain data (`Send`); the worker
        // instantiates its own Lua to run a matching plugin's `peek()`.
        let plugin_regs = self.plugins.previewer_regs();
        self.preview.rx = Some(spawn_worker(move |tx| {
            let msg = App::build_preview_content_with_plugins(request_id, path, opts, &plugin_regs);
            let _ = tx.send(msg);
        }));
    }
}

impl App {
    pub(crate) fn pump_preview_progress(&mut self) {
        if self.preview.rx.is_none() {
            return;
        }
        match pump_once(&mut self.preview.rx) {
            Some(PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb,
            }) => {
                if request_id == self.preview.request_id {
                    self.preview.target_path = Some(path.clone());
                    if image_rgb.is_none()
                        && !Self::is_image_file(&path)
                        && !Self::is_video_file(&path)
                    {
                        self.preview.cache
                            .insert(path.clone(), (lines.clone(), line_kinds.clone(), footer.clone()));
                    }
                    self.preview.lines = lines;
                    self.preview.line_kinds = line_kinds;
                    self.preview.footer = footer;
                    if let Some((ref rgb, w, h)) = image_rgb {
                        self.preview.image_png = App::encode_rgb_to_png(rgb, w, h);
                    } else {
                        self.preview.image_png = None;
                    }
                    self.preview.image_rgb = image_rgb;
                    self.preview.pending = false;
                    self.preview.scroll_offset = 0;
                }
            }
            Some(PreviewContentMsg::Failed {
                request_id,
                path,
                message,
            }) => {
                if request_id == self.preview.request_id {
                    self.preview.target_path = Some(path);
                    self.preview.lines = vec![message];
                    self.preview.line_kinds = vec![PreviewLineKind::Plain];
                    self.preview.footer = None;
                self.preview.image_rgb = None;
                self.preview.image_png = None;
                    self.preview.pending = false;
                    self.preview.scroll_offset = 0;
                }
            }
            None => {
                // `pump_once` drops the receiver when the worker disconnected
                // without sending; stop showing the pending state in that case.
                if self.preview.rx.is_none() {
                    self.preview.pending = false;
                }
            }
        }
    }

    /// Preview dispatch entry for the worker thread: a matching plugin
    /// previewer wins over every built-in except directory listings; on
    /// plugin error the failure is shown (not silently swallowed) so broken
    /// plugins are debuggable.
    fn build_preview_content_with_plugins(
        request_id: u64,
        path: PathBuf,
        opts: PreviewBuildOptions,
        regs: &[crate::plugin::PreviewerReg],
    ) -> PreviewContentMsg {
        if !path.is_dir()
            && let Some(reg) = crate::plugin::preview::match_previewer(regs, &path)
        {
            return match crate::plugin::preview::run_peek(reg, &path, 1000) {
                Ok((lines, footer)) => PreviewContentMsg::Ready {
                    request_id,
                    path,
                    line_kinds: vec![PreviewLineKind::Plain; lines.len()],
                    lines,
                    footer: footer.or_else(|| Some(format!("{} preview", reg.plugin))),
                    image_rgb: None,
                },
                Err(message) => PreviewContentMsg::Failed {
                    request_id,
                    path,
                    message: format!("[plugin {}: {}]", reg.plugin, message),
                },
            };
        }
        Self::build_preview_content(request_id, path, opts)
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
            use_timg,
            use_pdftotext,
            use_glow,
            use_doxx,
            use_xleak,
            use_sqlite3,
            use_sox,
            use_mmdflux,
            use_links,
            use_hexyl,
            use_zip_list,
            use_tar_list,
            use_7z_list,
            use_rar_list,
            pane_cols,
            pane_rows,
            show_icons,
            nerd_font_active,
            theme_id,
            filename_color_mode,
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
                let file_name = crate::util::classify::display_name(entry_path.as_path());
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

                // Honor the filename-color mode for regular files: the icon keeps
                // its color while the name uses the theme's normal text color.
                // Folders, symlinks, and executables keep their status colors.
                let mut icon = None;
                if !is_dir
                    && !is_symlink
                    && !is_executable
                    && !matches!(filename_color_mode, FilenameColorMode::Full)
                {
                    if show_icons && !icon_glyph.is_empty() {
                        icon = Some(PreviewIconSpan {
                            len: icon_prefix.len(),
                            fg: icon_style.fg,
                        });
                    }
                    style = style.fg(spec.text_normal);
                }

                if is_hidden {
                    style = style.add_modifier(Modifier::DIM);
                }

                line_kinds.push(PreviewLineKind::Styled {
                    fg: style.fg,
                    bold: style.add_modifier.contains(Modifier::BOLD),
                    dim: style.add_modifier.contains(Modifier::DIM),
                    icon,
                });
            }

            if entries.is_empty() {
                entries.push("[empty folder]".to_string());
                line_kinds.push(PreviewLineKind::Plain);
            }

            let footer = App::compute_total_display_bytes(&path, None)
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

        let protocol = App::terminal_image_protocol().0;
        let native_pane = matches!(
            protocol,
            crate::integration::probe::TerminalImageProtocol::Kitty
                | crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                | crate::integration::probe::TerminalImageProtocol::Sixel
        );

        // Videos on kitty-protocol terminals play live in the pane (see
        // app_video.rs): return placeholder text only; the run loop starts
        // timg playback once this message lands and the selection settles.
        if use_timg
            && matches!(protocol, crate::integration::probe::TerminalImageProtocol::Kitty)
            && App::is_video_file(&path)
        {
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines: vec!["[video]".to_string()],
                line_kinds: vec![PreviewLineKind::Plain],
                footer,
                image_rgb: None,
            };
        }

        // timg renders images and video first-frames as capturable quarter-block
        // ANSI text; on any failure fall through to the built-in renderers.
        // Images are only routed here when no native pixel protocol is
        // available — otherwise the in-process decode below feeds the sharper
        // kitty/iterm2/sixel pane renderer.
        if use_timg
            && (App::is_video_file(&path) || (!native_pane && App::is_image_file(&path)))
            && let Some(lines) = App::render_preview_with_timg(&path, pane_cols, pane_rows) {
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
            // Formats the `image` crate can't decode (HEIC, TIFF, ...): timg's
            // GraphicsMagick backend can still render them as block art.
            if image_rgb.is_none()
                && use_timg
                && let Some(lines) = App::render_preview_with_timg(&path, pane_cols, pane_rows) {
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

        // Integration-backed document/database/media/archive previews: each
        // tool emits capturable text or ANSI and returns early on success; a
        // missing, disabled, or failing tool falls through to the generic
        // binary/text handling below. The doxx/xleak branches must precede the
        // archive listing — .docx/.xlsx/.ods are zip-based.
        if use_pdftotext
            && App::is_pdf_file(&path)
            && let Some(lines) = App::capture_preview_tool(
                Command::new("pdftotext").args(["-l", "8", "-layout"]).arg(&path).arg("-"),
                400,
            )
        {
            return Self::tool_preview_ready(request_id, path, lines, "pdftotext");
        }
        if use_doxx
            && App::is_docx_file(&path)
            && let Some(lines) = App::capture_preview_tool(
                Command::new("doxx")
                    .arg(&path)
                    .args(["--export", "ansi", "-w"])
                    .arg(pane_cols.to_string()),
                400,
            )
        {
            return Self::tool_preview_ready(request_id, path, lines, "doxx");
        }
        if use_xleak
            && App::is_excel_file(&path)
            && let Some(lines) = App::capture_preview_tool(
                Command::new("xleak").arg(&path).args(["-n", "100"]),
                400,
            )
        {
            return Self::tool_preview_ready(request_id, path, lines, "xleak");
        }
        if use_sqlite3
            && App::is_sqlite_db_file(&path)
            && let Some(lines) = App::render_sqlite_preview_lines(&path)
        {
            return Self::tool_preview_ready(request_id, path, lines, "sqlite3");
        }
        if use_sox
            && App::is_audio_file(&path)
            && let Some(lines) = App::capture_preview_tool(Command::new("soxi").arg(&path), 60)
        {
            return Self::tool_preview_ready(request_id, path, lines, "soxi");
        }
        if use_mmdflux
            && App::is_mermaid_file(&path)
            && let Some(lines) = App::capture_preview_tool(
                // Explicit --color skips mmdflux's OSC 11 tty query, whose
                // reply sb's own event loop would swallow, hanging mmdflux.
                Command::new("mmdflux").args(["-f", "text", "--color", "always"]).arg(&path),
                400,
            )
        {
            return Self::tool_preview_ready(request_id, path, lines, "mmdflux");
        }
        if use_links
            && App::is_html_file(&path)
            && let Some(lines) = App::capture_preview_tool(
                Command::new("links").arg("-dump").arg(&path),
                400,
            )
        {
            return Self::tool_preview_ready(request_id, path, lines, "links");
        }
        // `-s dark` forces ANSI styling when stdout is a pipe (glow's auto
        // style degrades to plain text there).
        if use_glow
            && App::is_markdown_file(&path)
            && let Some(lines) = App::capture_preview_tool(
                Command::new("glow")
                    .args(["-s", "dark", "-w"])
                    .arg(pane_cols.min(120).to_string())
                    .arg(&path),
                400,
            )
        {
            return Self::tool_preview_ready(request_id, path, lines, "glow");
        }
        if let Some(kind) = App::archive_kind(&path) {
            let listed = match kind {
                crate::ArchiveKind::Zip if use_zip_list => App::capture_preview_tool(
                    Command::new("unzip").arg("-l").arg(&path),
                    220,
                ),
                crate::ArchiveKind::Tar if use_tar_list => App::capture_preview_tool(
                    Command::new("tar").arg("-tvf").arg(&path),
                    220,
                ),
                crate::ArchiveKind::SevenZip if use_7z_list => Self::seven_zip_tool()
                    .and_then(|tool| {
                        App::capture_preview_tool(Command::new(tool).arg("l").arg(&path), 220)
                    }),
                crate::ArchiveKind::Rar if use_rar_list => Self::rar_tool().and_then(|tool| {
                    App::capture_preview_tool(Command::new(tool).arg("l").arg(&path), 220)
                }),
                _ => None,
            };
            if let Some(lines) = listed {
                return Self::tool_preview_ready(request_id, path, lines, "archive listing");
            }
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
            if use_hexyl
                && let Some(hex) = App::capture_preview_tool(
                    Command::new("hexyl").args(["-n", "1024", "--color=always"]).arg(&path),
                    80,
                )
            {
                lines.push(String::new());
                lines.extend(hex);
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

    /// Runs a preview tool that prints capturable text/ANSI to stdout and
    /// returns up to `max_lines` lines; None on spawn failure, non-zero exit,
    /// or blank output so callers fall through to the next renderer.
    fn capture_preview_tool(cmd: &mut Command, max_lines: usize) -> Option<Vec<String>> {
        // Detached from the tty: even with stdin null some tools (mmdflux)
        // open /dev/tty for a terminal query whose reply sb's event loop
        // eats, stalling the tool — and flip the tty's termios while at it.
        crate::util::command::detach_from_tty(cmd);
        let out = cmd.stdin(Stdio::null()).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<String> = text.lines().take(max_lines).map(|s| s.to_string()).collect();
        if lines.iter().all(|l| l.trim().is_empty()) {
            return None;
        }
        Some(lines)
    }

    /// Wraps captured tool output into a Ready message with the standard size
    /// footer, tagging which integration produced the preview.
    fn tool_preview_ready(
        request_id: u64,
        path: PathBuf,
        lines: Vec<String>,
        tool: &str,
    ) -> PreviewContentMsg {
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let footer = Some(format!("{} · Size: {}", tool, App::format_size(size)));
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

    /// Renders an image or a video's first frame to ANSI quarter-block text
    /// sized to the preview pane. `-pq` keeps the output plain capturable text
    /// (never kitty/sixel escapes); returns None on any failure so callers can
    /// fall back to the built-in renderers.
    fn render_preview_with_timg(path: &PathBuf, cols: u16, rows: u16) -> Option<Vec<String>> {
        let out = Command::new("timg")
            .arg(format!("-g{}x{}", cols.max(4), rows.max(2)))
            .args(["-pq", "--frames=1", "--loops=1"])
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        if lines.iter().all(|l| l.trim().is_empty()) {
            return None;
        }
        Some(lines)
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
        // `$tool` is expanded unquoted on purpose: it is a trusted, hardcoded
        // invocation that may carry flags (e.g. "timg --loops=1").
        let script = r#"
tool="$1"
img="$2"
clear
$tool -- "$img"
# Drain stray terminal-query replies left by the viewer (cell size,
# background color) so they don't count as the dismissing keypress.
while IFS= read -rsn1 -t 0.05 _; do :; done
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
