use crate::{App, DualPanelSide, EntryRenderConfig, PreviewPaneFocus, ViewMode};

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
                self.right_dir = self.current_dir.clone();
                self.right_selected_index = 0;
                self.right_table_state = ratatui::widgets::TableState::default();
                self.right_sort_mode = self.sort_mode;
                self.right_show_hidden = self.show_hidden;
                self.active_panel = DualPanelSide::Left;
                let _ = self.refresh_right_panel_entries();
            }
            ViewMode::DualPanel => {
                // Preserve the active panel's directory when returning to normal mode
                self.current_dir = self.active_panel_dir();
                self.view_mode = ViewMode::Normal;
                self.right_dir = std::path::PathBuf::new();
                self.right_entries.clear();
                self.right_tree_row_prefixes.clear();
                self.right_entry_render_cache.clear();
                self.right_selected_index = 0;
                self.right_marked_indices.clear();
                self.clear_selected_total_size_state_for(DualPanelSide::Right);
                self.right_status_message.clear();
                self.right_table_state = ratatui::widgets::TableState::default();
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
                &self.right_dir,
                self.right_show_hidden,
                self.right_sort_mode,
                folder_size_cache,
                &self.tree_expansion_levels,
            )?;
            self.right_tree_row_prefixes = rows.iter().map(|row| row.prefix.clone()).collect();
            rows.into_iter().map(|row| row.entry).collect()
        } else {
            let mut direct_entries: Vec<_> = std::fs::read_dir(&self.right_dir)?
                .filter_map(|res| res.ok())
                .filter(|e| {
                    self.right_show_hidden || !e.file_name().to_string_lossy().starts_with('.')
                })
                .collect();
            Self::sort_entries_by_mode(&mut direct_entries, self.right_sort_mode, folder_size_cache);
            self.right_tree_row_prefixes = vec![String::new(); direct_entries.len()];
            direct_entries
        };
        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
        };
        let uid_cache = App::build_uid_cache(&entries);
        let gid_cache = App::build_gid_cache(&entries);
        self.right_entry_render_cache = entries
            .iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        self.right_entries = entries;
        if self.folder_size_enabled {
            self.apply_cached_folder_size_columns();
            self.start_folder_size_scan();
            self.refresh_current_dir_free_space();
            self.start_current_dir_total_size_scan();
        }
        self.right_marked_indices.clear();
        self.clear_selected_total_size_state_for(DualPanelSide::Right);
        if self.right_entries.is_empty() {
            self.right_selected_index = 0;
            self.right_table_state.select(None);
        } else {
            self.right_selected_index = self.right_selected_index.min(self.right_entries.len() - 1);
            self.right_table_state.select(Some(self.right_selected_index));
        }
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

        if !Self::is_image_file(&path) {
            if let Some(cached) = self.preview_cache.get(&path).cloned() {
                self.preview_target_path = Some(path);
                self.preview_lines = cached.0;
                self.preview_line_kinds = cached.1;
                self.preview_footer = cached.2;
                self.preview_pending = false;
                self.preview_scroll_offset = 0;
                return;
            }
        }

        self.preview_request_id = self.preview_request_id.saturating_add(1);
        let request_id = self.preview_request_id;
        self.preview_target_path = Some(path.clone());
        self.preview_pending = true;
        self.preview_scroll_offset = 0;
        self.preview_lines = vec!["Loading preview...".to_string()];
        self.preview_line_kinds = vec![crate::PreviewLineKind::Plain];
        self.preview_footer = None;

        let use_bat = Self::integration_availability_and_detail("bat").0;
        let use_file = Self::integration_availability_and_detail("file").0;
        let use_resvg = self.integration_active("resvg");
        let show_icons = self.show_icons;
        let nerd_font_active = self.nerd_font_active;
        let (tx, rx) = std::sync::mpsc::channel();
        self.preview_rx = Some(rx);
        std::thread::spawn(move || {
            let msg = App::build_preview_content(
                request_id,
                path,
                use_bat,
                use_file,
                use_resvg,
                show_icons,
                nerd_font_active,
            );
            let _ = tx.send(msg);
        });
    }
}
