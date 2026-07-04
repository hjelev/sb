use ratatui::layout::{Constraint, Direction, Layout, Rect};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::ui;
use crate::util::list::{cursor_down, cursor_up};
use crate::{App, AppMode, DualPanelSide, InternalSearchScope, PreviewPaneFocus};

impl App {

    pub(crate) fn panel_tab_hit_test(relative_x: u16, active: u8, avail_width: u16) -> Option<u8> {
        ui::panels::panel_tab_hit_test(relative_x, active, avail_width)
    }

    pub(crate) fn tabbed_overlay_close_area(popup_area: Rect) -> Rect {
        Rect::new(
            popup_area.x + popup_area.width.saturating_sub(2),
            popup_area.y,
            1,
            1,
        )
    }

    pub(crate) fn primary_content_area(area: Rect) -> Rect {
        Layout::default()
            .constraints([Constraint::Min(3), Constraint::Length(2)])
            .split(area)[0]
    }

    pub(crate) fn tab_overlay_anchor(area: Rect) -> Rect {
        let area = Self::primary_content_area(area);
        let anchor_w = (area.width * 5 / 6).max(50).min(area.width);
        let anchor_h = (area.height * 5 / 6).max(12).min(area.height);
        Rect::new(
            area.x + (area.width.saturating_sub(anchor_w)) / 2,
            area.y + (area.height.saturating_sub(anchor_h)) / 2,
            anchor_w,
            anchor_h,
        )
    }

    pub(crate) fn open_panel_tab(&mut self, tab: u8) {
        if tab == self.panel_tab
            && matches!(
                (tab, self.mode),
                (0, AppMode::Help)
                    | (1, AppMode::InternalSearch)
                    | (2, AppMode::Bookmarks)
                    | (3, AppMode::SshPicker)
                    | (4, AppMode::SortMenu)
                    | (5, AppMode::Integrations)
                    | (6, AppMode::Themes)
                    | (7, AppMode::Settings)
                    | (8, AppMode::Shortcuts)
            )
        {
            return;
        }

        match tab {
            0 => {
                self.panel_tab = 0;
                self.help_scroll_offset = 0;
                self.mode = AppMode::Help;
            }
            1 => {
                self.panel_tab = 1;
                self.start_internal_search();
            }
            2 => {
                self.panel_tab = 2;
                self.refresh_bookmarks_cache();
                self.mode = AppMode::Bookmarks;
            }
            3 => {
                self.panel_tab = 3;
                self.refresh_remote_entries();
                self.mode = AppMode::SshPicker;
            }
            4 => {
                self.begin_sort_menu();
            }
            5 => {
                self.integration_selected = 0;
                self.reset_integration_search();
                self.refresh_integration_rows_cache();
                self.panel_tab = 5;
                self.mode = AppMode::Integrations;
            }
            6 => {
                self.theme_selected = ui::theme::themes()
                    .iter()
                    .position(|theme| theme.id == self.active_theme)
                    .unwrap_or(0);
                self.panel_tab = 6;
                self.theme_panel_nerd_selected = false;
                self.theme_panel_color_selected = false;
                self.theme_panel_clock_selected = false;
                self.mode = AppMode::Themes;
            }
            7 => {
                self.settings_selected = 0;
                self.panel_tab = 7;
                self.mode = AppMode::Settings;
                self.maybe_check_api_key();
            }
            8 => {
                self.shortcuts_selected = 0;
                self.shortcut_capture = false;
                self.panel_tab = 8;
                self.mode = AppMode::Shortcuts;
            }
            _ => {}
        }
    }

    pub(crate) fn close_tabbed_overlay(&mut self) {
        match self.mode {
            AppMode::InternalSearch => {
                self.cancel_internal_search_candidate_scan();
                self.cancel_internal_search_content_request();
                self.clear_input_edit();
                self.mode = AppMode::Browsing;
            }
            AppMode::Help
            | AppMode::Bookmarks
            | AppMode::Integrations
            | AppMode::Themes
            | AppMode::SortMenu
            | AppMode::Settings
            | AppMode::Shortcuts
            | AppMode::SshPicker => {
                self.shortcut_capture = false;
                self.mode = AppMode::Browsing;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_tab_close_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if !matches!(
            self.mode,
            AppMode::InternalSearch
                | AppMode::Help
                | AppMode::Bookmarks
                | AppMode::Integrations
                | AppMode::Themes
                | AppMode::SortMenu
                | AppMode::Settings
                | AppMode::Shortcuts
                | AppMode::SshPicker
        ) {
            return false;
        }

        let popup_area = Self::tab_overlay_anchor(area);
        let close_area = Self::tabbed_overlay_close_area(popup_area);
        if row == close_area.y && column >= close_area.x && column < close_area.x + close_area.width {
            self.close_tabbed_overlay();
            return true;
        }

        false
    }

    pub(crate) fn handle_tab_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if !matches!(
            self.mode,
            AppMode::InternalSearch
                | AppMode::Help
                | AppMode::Bookmarks
                | AppMode::Integrations
                | AppMode::Themes
                | AppMode::SortMenu
                | AppMode::Settings
                | AppMode::Shortcuts
                | AppMode::SshPicker
        ) {
            return false;
        }

        let popup_area = Self::tab_overlay_anchor(area);
        if row != popup_area.y || column <= popup_area.x || column >= popup_area.x + popup_area.width.saturating_sub(1) {
            return false;
        }

        let relative_x = column.saturating_sub(popup_area.x + 1);
        if let Some(tab) = Self::panel_tab_hit_test(relative_x, self.panel_tab, popup_area.width.saturating_sub(3)) {
            self.open_panel_tab(tab);
            return true;
        }

        false
    }

    pub(crate) fn handle_confirm_delete_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if self.mode != AppMode::ConfirmDelete {
            return false;
        }

        let folder_count = self.confirm_delete_targets.iter().filter(|t| t.is_dir).count();
        let file_count = self.confirm_delete_targets.len() - folder_count;
        let title = ui::dialogs::confirm_delete_title(file_count, folder_count);
        let confirm_area = ui::dialogs::confirm_delete_dialog_area(area, &title);
        let Some((button_area, confirm_start, confirm_w, cancel_start, cancel_w)) =
            ui::dialogs::confirm_delete_button_layout(confirm_area)
        else {
            return false;
        };

        if row != button_area.y {
            return false;
        }

        if column >= confirm_start && column < confirm_start + confirm_w {
            self.confirm_delete_button_focus = 0;
            self.confirm_delete_selected_targets();
            return true;
        }
        if column >= cancel_start && column < cancel_start + cancel_w {
            self.confirm_delete_button_focus = 1;
            self.mode = AppMode::Browsing;
            return true;
        }

        false
    }

    pub(crate) fn handle_confirm_integration_install_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if self.mode != AppMode::ConfirmIntegrationInstall {
            return false;
        }

        let Some((button_area, ok_start, ok_w, cancel_start, cancel_w)) =
            self.confirm_integration_install_button_layout(area)
        else {
            return false;
        };

        if row != button_area.y {
            return false;
        }

        if column >= ok_start && column < ok_start + ok_w {
            self.confirm_integration_install_button_focus = 0;
            return self.confirm_integration_install().is_ok();
        }
        if column >= cancel_start && column < cancel_start + cancel_w {
            self.confirm_integration_install_button_focus = 1;
            self.mode = AppMode::Integrations;
            self.clear_integration_install_prompt();
            self.set_status("integration install cancelled");
            return true;
        }

        false
    }

    pub(crate) fn confirm_integration_install_msg_lines(&self) -> Vec<String> {
        let key = self
            .integration_install_key
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string());
        let package = self
            .integration_install_package
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string());
        let brew_display = self
            .integration_install_brew_path
            .clone()
            .unwrap_or_else(|| "brew (not found)".to_string());

        ui::dialogs::confirm_integration_install_msg_lines(
            &key,
            &package,
            &brew_display,
            self.integration_install_brew_path.is_none(),
        )
    }

    pub(crate) fn confirm_integration_install_dialog_area(&self, area: Rect) -> Rect {
        let msg_lines = self.confirm_integration_install_msg_lines();
        ui::dialogs::confirm_integration_install_dialog_area(area, &msg_lines)
    }

    pub(crate) fn confirm_integration_install_button_layout(
        &self,
        area: Rect,
    ) -> Option<(Rect, u16, u16, u16, u16)> {
        let confirm_area = self.confirm_integration_install_dialog_area(area);
        ui::dialogs::confirm_ok_cancel_button_layout(confirm_area)
    }

    pub(crate) fn inner_with_borders(area: Rect) -> Rect {
        Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        )
    }

    pub(crate) fn internal_search_header_rows(&self) -> usize {
        let mut rows = 0usize;
        if self.search.candidates_pending || self.search.candidates_truncated {
            rows += 1;
        }

        if self.search.scope == InternalSearchScope::Content {
            rows += 1; // limits summary
            if self.search.limits_menu_open {
                rows += 4; // 3 editable rows + helper line
            } else {
                rows += 1; // open editor hint
            }
            if self.search.content_pending {
                rows += 1;
            }
            if self.search.content_limit_note.is_some() {
                rows += 1;
            }
        }

        rows
    }

    pub(crate) fn clickable_key_from_tabbed_row(
        &mut self,
        column: u16,
        row: u16,
        area: Rect,
    ) -> Option<KeyEvent> {
        match self.mode {
            AppMode::InternalSearch => {
                if self.search.results.is_empty() {
                    return None;
                }

                let popup_area = Self::tab_overlay_anchor(area);
                let popup_inner = Self::inner_with_borders(popup_area);
                let search_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(1),
                        Constraint::Length(2),
                    ])
                    .split(popup_inner);
                let body_area = search_layout[1];

                if row < body_area.y || row >= body_area.y + body_area.height {
                    return None;
                }
                if column < body_area.x || column >= body_area.x + body_area.width {
                    return None;
                }

                let header_rows = self.internal_search_header_rows();
                let regex_rows = usize::from(self.search.regex_error.is_some());
                let visible_rows = body_area.height as usize;
                let max_rows = visible_rows.saturating_sub(header_rows).max(1);
                let offset = if self.search.selected >= max_rows {
                    self.search.selected + 1 - max_rows
                } else {
                    0
                };

                let result_start_y = body_area
                    .y
                    .saturating_add((header_rows + regex_rows) as u16);
                if row < result_start_y {
                    return None;
                }

                let clicked_result_row = row.saturating_sub(result_start_y) as usize;
                let rendered_results = self
                    .search.results
                    .len()
                    .saturating_sub(offset)
                    .min(max_rows);
                if clicked_result_row >= rendered_results {
                    return None;
                }

                self.search.selected = offset + clicked_result_row;
                Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            }
            AppMode::Bookmarks => {
                let overlay = Self::tab_overlay_anchor(area);
                let bookmarks_len = self.bookmarks().len();
                if bookmarks_len == 0 {
                    return None;
                }

                let bm_w = (area.width * 2 / 3).max(50).min(overlay.width);
                let mut line_count = 1usize + bookmarks_len;
                line_count += 4; // trailing helper lines
                let bm_h = (line_count as u16 + 4).max(17).min(overlay.height);
                let bm_area = Rect::new(overlay.x, overlay.y, bm_w, bm_h);
                let bm_inner = Self::inner_with_borders(bm_area);
                let bm_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(bm_inner);
                let content = bm_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let line_idx = row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= bookmarks_len {
                    self.bookmark_selected = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }

                None
            }
            AppMode::Integrations => {
                let overlay = Self::tab_overlay_anchor(area);
                let integrations_len = self.integration_rows_cache.len();
                if integrations_len == 0 {
                    return None;
                }

                let int_w = (area.width * 5 / 6).max(70).min(overlay.width);
                let int_h = (integrations_len as u16 + 1 + 4).min(overlay.height);
                let int_area = Rect::new(overlay.x, overlay.y, int_w, int_h);
                let int_inner = Self::inner_with_borders(int_area);
                let int_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(int_inner);
                let content = int_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let visible_rows = content.height as usize;
                let selected_line = self.integration_selected + 1;
                let int_scroll = (selected_line + 1).saturating_sub(visible_rows);
                let line_idx = int_scroll + row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= integrations_len {
                    self.integration_selected = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
                }

                None
            }
            AppMode::SshPicker => {
                if self.remote_entries.is_empty() {
                    return None;
                }

                let overlay = Self::tab_overlay_anchor(area);
                let ssh_w = (area.width * 2 / 3).max(60).min(area.width);
                let ssh_popup_w = ssh_w.min(overlay.width);
                let lines_len = 1usize + self.remote_entries.len();
                let ssh_h = (lines_len as u16 + 4).max(8).min(overlay.height);
                let ssh_area = Rect::new(overlay.x, overlay.y, ssh_popup_w, ssh_h);
                let ssh_inner = Self::inner_with_borders(ssh_area);
                let ssh_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(ssh_inner);
                let content = ssh_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let line_idx = row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= self.remote_entries.len() {
                    self.ssh_picker_selection = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }

                None
            }
            AppMode::SortMenu => {
                let overlay = Self::tab_overlay_anchor(area);
                let options = Self::sort_mode_options();
                if options.is_empty() {
                    return None;
                }

                let sort_w = overlay.width;
                let line_count = 1usize + options.len();
                let sort_h = (line_count as u16 + 4).max(10).min(overlay.height);
                let sort_area = Rect::new(overlay.x, overlay.y, sort_w, sort_h);
                let sort_inner = Self::inner_with_borders(sort_area);
                let sort_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(sort_inner);
                let content = sort_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let line_idx = row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= options.len() {
                    self.sort_menu_selected = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }

                None
            }
            AppMode::Themes => {
                let overlay = Self::tab_overlay_anchor(area);
                let themes = crate::ui::theme::themes();
                // Body lines mirror render_themes_overlay: 0 blank, 1 Nerd Fonts,
                // 2 Filename colors, 3 Disable clock, 4 blank, then one row per
                // theme (no scroll offset).
                let line_count = 5 + themes.len();
                let theme_h = ((line_count as u16) + 7).max(12).min(overlay.height);
                let theme_area = Rect::new(overlay.x, overlay.y, overlay.width, theme_h);
                let theme_inner = Self::inner_with_borders(theme_area);
                let theme_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(theme_inner);
                let content = theme_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                // Move focus to the clicked row, then synthesize Enter so the
                // existing key handler applies/toggles the focused item.
                let offset = row.saturating_sub(content.y) as usize;
                match offset {
                    1 => {
                        self.theme_panel_nerd_selected = true;
                        self.theme_panel_color_selected = false;
                        self.theme_panel_clock_selected = false;
                    }
                    2 => {
                        self.theme_panel_nerd_selected = false;
                        self.theme_panel_color_selected = true;
                        self.theme_panel_clock_selected = false;
                    }
                    3 => {
                        self.theme_panel_nerd_selected = false;
                        self.theme_panel_color_selected = false;
                        self.theme_panel_clock_selected = true;
                    }
                    o if o >= 5 && (o - 5) < themes.len() => {
                        self.theme_panel_nerd_selected = false;
                        self.theme_panel_color_selected = false;
                        self.theme_panel_clock_selected = false;
                        self.theme_selected = o - 5;
                    }
                    _ => return None, // blank separator rows
                }
                Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            }
            _ => None,
        }
    }

    pub(crate) fn handle_mouse_scroll(&mut self, scroll_up: bool) {
        match self.mode {
            AppMode::Browsing => {
                if self.preview_focus_is_preview() {
                    if scroll_up {
                        self.preview_scroll_up(1);
                    } else {
                        self.preview_scroll_down(1);
                    }
                } else if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
                    if !self.right.entries.is_empty() {
                        let max_idx = self.right.entries.len() - 1;
                        let next = if scroll_up {
                            self.right.selected_index.saturating_sub(1)
                        } else {
                            (self.right.selected_index + 1).min(max_idx)
                        };
                        self.right.selected_index = next;
                        self.right.table_state.select(Some(next));
                    }
                } else {
                    let delta = if scroll_up { -1 } else { 1 };
                    self.move_selection_delta(delta);
                }
            }
            AppMode::Help => {
                if scroll_up {
                    self.help_scroll_offset = self.help_scroll_offset.saturating_sub(1);
                } else {
                    self.help_scroll_offset = (self.help_scroll_offset + 1).min(self.help_max_offset);
                }
            }
            AppMode::InternalSearch => {
                if self.search.limits_menu_open {
                    if scroll_up {
                        cursor_up(&mut self.search.limits_selected);
                    } else {
                        cursor_down(&mut self.search.limits_selected, 3);
                    }
                } else if !self.search.results.is_empty() {
                    if scroll_up {
                        cursor_up(&mut self.search.selected);
                    } else {
                        cursor_down(&mut self.search.selected, self.search.results.len());
                    }
                }
            }
            AppMode::Bookmarks => {
                if scroll_up {
                    cursor_up(&mut self.bookmark_selected);
                } else {
                    let len = self.bookmarks().len();
                    cursor_down(&mut self.bookmark_selected, len);
                }
            }
            AppMode::Integrations => {
                if scroll_up {
                    cursor_up(&mut self.integration_selected);
                } else {
                    cursor_down(&mut self.integration_selected, self.integration_rows_cache.len());
                }
            }
            AppMode::Settings => {
                if scroll_up {
                    cursor_up(&mut self.settings_selected);
                } else {
                    cursor_down(&mut self.settings_selected, 4);
                }
            }
            AppMode::Shortcuts => {
                if scroll_up {
                    cursor_up(&mut self.shortcuts_selected);
                } else {
                    cursor_down(&mut self.shortcuts_selected, crate::util::keymap::ACTIONS.len());
                }
            }
            AppMode::SortMenu => {
                if scroll_up {
                    cursor_up(&mut self.sort_menu_selected);
                } else {
                    cursor_down(&mut self.sort_menu_selected, Self::sort_mode_options().len());
                }
            }
            AppMode::SshPicker => {
                if scroll_up {
                    cursor_up(&mut self.ssh_picker_selection);
                } else {
                    cursor_down(&mut self.ssh_picker_selection, self.remote_entries.len());
                }
            }
            AppMode::Themes => {
                // Mirror the keyboard Up/Down focus order:
                // Nerd Fonts → Filename colors → Disable clock → theme list.
                if scroll_up {
                    if self.theme_panel_nerd_selected {
                        // already at the top row
                    } else if self.theme_panel_color_selected {
                        self.theme_panel_color_selected = false;
                        self.theme_panel_nerd_selected = true;
                    } else if self.theme_panel_clock_selected {
                        self.theme_panel_clock_selected = false;
                        self.theme_panel_color_selected = true;
                    } else if self.theme_selected == 0 {
                        self.theme_panel_clock_selected = true;
                    } else {
                        self.theme_selected -= 1;
                    }
                } else if self.theme_panel_nerd_selected {
                    self.theme_panel_nerd_selected = false;
                    self.theme_panel_color_selected = true;
                } else if self.theme_panel_color_selected {
                    self.theme_panel_color_selected = false;
                    self.theme_panel_clock_selected = true;
                } else if self.theme_panel_clock_selected {
                    self.theme_panel_clock_selected = false;
                } else {
                    let max_idx = crate::ui::theme::themes().len().saturating_sub(1);
                    self.theme_selected = (self.theme_selected + 1).min(max_idx);
                }
            }
            AppMode::ConfirmDelete => {
                if scroll_up {
                    self.confirm_delete_scroll_offset = self.confirm_delete_scroll_offset.saturating_sub(1);
                } else {
                    self.confirm_delete_scroll_offset =
                        (self.confirm_delete_scroll_offset + 1).min(self.confirm_delete_max_offset);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn main_table_and_list_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
        if self.mode != AppMode::Browsing {
            return None;
        }

        let footer_height = if self.is_preview_mode() || self.is_dual_panel_mode() { 1 } else { 2 };
        let header_reserved_rows = if self.is_preview_mode() || self.is_dual_panel_mode() { 1 } else { 2 };
        let chunks = Layout::default()
            .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
            .split(area);

        let content_area = Rect::new(
            chunks[0].x,
            chunks[0].y + header_reserved_rows,
            chunks[0].width,
            chunks[0].height.saturating_sub(header_reserved_rows),
        );

        let list_frame_area = if self.is_preview_mode() {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(33), Constraint::Percentage(67)])
                .split(content_area)[0]
        } else if self.is_dual_panel_mode() {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(content_area)[0]
        } else {
            content_area
        };

        let table_area = if self.is_preview_mode() || self.is_dual_panel_mode() {
            Rect::new(
                list_frame_area.x + 1,
                list_frame_area.y + 1,
                list_frame_area.width.saturating_sub(2),
                list_frame_area.height.saturating_sub(2),
            )
        } else {
            content_area
        };

        if table_area.height == 0 || table_area.width == 0 {
            return None;
        }

        let needs_scroll = self.left.entries.len() > table_area.height as usize;
        let can_draw_scrollbar = self.mode_shows_main_scrollbar() && table_area.width > 2 && needs_scroll;
        let list_area = if can_draw_scrollbar {
            Rect::new(
                table_area.x,
                table_area.y,
                table_area.width.saturating_sub(1),
                table_area.height,
            )
        } else {
            table_area
        };

        Some((table_area, list_area))
    }

    pub(crate) fn preview_pane_frame_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
        if !self.is_preview_mode() || !matches!(self.mode, AppMode::Browsing | AppMode::PathEditing) {
            return None;
        }

        let footer_height = 1;
        let header_reserved_rows = 1;
        let chunks = Layout::default()
            .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
            .split(area);

        let content_area = Rect::new(
            chunks[0].x,
            chunks[0].y + header_reserved_rows,
            chunks[0].width,
            chunks[0].height.saturating_sub(header_reserved_rows),
        );
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Percentage(67)])
            .split(content_area);
        Some((split[0], split[1]))
    }

    pub(crate) fn handle_preview_pane_tab_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        let Some((folder_area, preview_area)) = self.preview_pane_frame_areas(area) else {
            return false;
        };

        let in_folder = column >= folder_area.x
            && column < folder_area.x + folder_area.width
            && row >= folder_area.y
            && row < folder_area.y + folder_area.height;
        let in_preview = column >= preview_area.x
            && column < preview_area.x + preview_area.width
            && row >= preview_area.y
            && row < preview_area.y + preview_area.height;

        if in_folder {
            self.preview_pane_focus = PreviewPaneFocus::Folder;
            return false;
        }

        if in_preview {
            self.preview_pane_focus = PreviewPaneFocus::Preview;
            return true;
        }

        false
    }

    pub(crate) fn main_table_scrollbar_area(&self, area: Rect) -> Option<Rect> {
        let (table_area, list_area) = self.main_table_and_list_areas(area)?;
        if list_area.width >= table_area.width || list_area.height == 0 {
            return None;
        }

        Some(Rect::new(
            list_area.x + list_area.width,
            list_area.y,
            1,
            list_area.height,
        ))
    }

    /// Hit-test a click against the footer shortcut pills (rebuilt each render
    /// for both the main footer and the tabbed overlay footers). A hit returns
    /// the stored key event so clicking a pill behaves exactly like pressing it.
    fn handle_footer_shortcut_click(&self, column: u16, row: u16) -> Option<KeyEvent> {
        for &(event, x0, x1, y) in &self.footer_shortcut_zones {
            if row == y && column >= x0 && column < x1 {
                return Some(event);
            }
        }
        None
    }

    pub(crate) fn handle_main_list_click(&mut self, column: u16, row: u16, area: Rect) -> Option<KeyEvent> {
        let (_, list_area) = self.main_table_and_list_areas(area)?;
        if list_area.width == 0 || list_area.height == 0 {
            return None;
        }
        if column < list_area.x
            || column >= list_area.x + list_area.width
            || row < list_area.y
            || row >= list_area.y + list_area.height
        {
            return None;
        }

        let row_rel = row.saturating_sub(list_area.y) as usize;
        let target_idx = self.left.table_state.offset().saturating_add(row_rel);
        if target_idx >= self.left.entries.len() {
            return None;
        }

        self.left.selected_index = target_idx;
        self.left.table_state.select(Some(target_idx));
        if self.is_dual_panel_mode() {
            self.active_panel = DualPanelSide::Left;
        }

        let is_double_click = Self::update_list_double_click_state(
            &mut self.left.list_last_click,
            &self.left.dir,
            target_idx,
        );

        if is_double_click {
            Some(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
        } else {
            None
        }
    }

    pub(crate) fn scroll_main_list_from_scrollbar_row(&mut self, area: Rect, row: u16, grab_offset: u16) {
        let Some(sb_area) = self.main_table_scrollbar_area(area) else {
            return;
        };
        let track_h = sb_area.height as usize;
        if track_h == 0 || self.left.entries.is_empty() {
            return;
        }
        let visible_rows = sb_area.height.max(1) as usize;
        let total_rows = self.left.entries.len();
        let max_scroll = total_rows.saturating_sub(visible_rows);
        if max_scroll == 0 {
            return;
        }

        let (_, thumb_h) = ui::scrollbar::scrollbar_thumb(total_rows, visible_rows, 0, track_h);
        let scroll_space = track_h.saturating_sub(thumb_h);
        if scroll_space == 0 {
            return;
        }

        let row_rel = row.saturating_sub(sb_area.y) as usize;
        let thumb_top = row_rel.saturating_sub(grab_offset as usize).min(scroll_space);
        let target_offset = (thumb_top * max_scroll + (scroll_space / 2)) / scroll_space;
        let target_index = target_offset.min(self.left.entries.len().saturating_sub(1));
        self.left.selected_index = target_index;
        self.left.table_state.select(Some(target_index));
        if self.is_dual_panel_mode() {
            self.active_panel = DualPanelSide::Left;
        }
    }

    pub(crate) fn right_table_and_list_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
        let (_, right_frame_area) = self.dual_panel_frame_areas(area)?;

        let table_area = Rect::new(
            right_frame_area.x + 1,
            right_frame_area.y + 1,
            right_frame_area.width.saturating_sub(2),
            right_frame_area.height.saturating_sub(2),
        );

        if table_area.height == 0 || table_area.width == 0 {
            return None;
        }

        let needs_scroll = self.right.entries.len() > table_area.height as usize;
        let can_draw_scrollbar = table_area.width > 2 && needs_scroll;
        let list_area = if can_draw_scrollbar {
            Rect::new(
                table_area.x,
                table_area.y,
                table_area.width.saturating_sub(1),
                table_area.height,
            )
        } else {
            table_area
        };

        Some((table_area, list_area))
    }

    pub(crate) fn right_table_scrollbar_area(&self, area: Rect) -> Option<Rect> {
        let (table_area, list_area) = self.right_table_and_list_areas(area)?;
        if list_area.width >= table_area.width || list_area.height == 0 {
            return None;
        }

        Some(Rect::new(
            list_area.x + list_area.width,
            list_area.y,
            1,
            list_area.height,
        ))
    }

    pub(crate) fn handle_right_list_click(&mut self, column: u16, row: u16, area: Rect) -> Option<KeyEvent> {
        let (_, list_area) = self.right_table_and_list_areas(area)?;
        if list_area.width == 0 || list_area.height == 0 {
            return None;
        }
        if column < list_area.x
            || column >= list_area.x + list_area.width
            || row < list_area.y
            || row >= list_area.y + list_area.height
        {
            return None;
        }

        let row_rel = row.saturating_sub(list_area.y) as usize;
        let target_idx = self.right.table_state.offset().saturating_add(row_rel);
        if target_idx >= self.right.entries.len() {
            return None;
        }

        self.right.selected_index = target_idx;
        self.right.table_state.select(Some(target_idx));
        self.active_panel = DualPanelSide::Right;

        let is_double_click = Self::update_list_double_click_state(
            &mut self.right.list_last_click,
            &self.right.dir,
            target_idx,
        );

        if is_double_click {
            Some(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
        } else {
            None
        }
    }

    pub(crate) fn scroll_right_list_from_scrollbar_row(&mut self, area: Rect, row: u16, grab_offset: u16) {
        let Some(sb_area) = self.right_table_scrollbar_area(area) else {
            return;
        };
        let track_h = sb_area.height as usize;
        if track_h == 0 || self.right.entries.is_empty() {
            return;
        }
        let visible_rows = sb_area.height.max(1) as usize;
        let total_rows = self.right.entries.len();
        let max_scroll = total_rows.saturating_sub(visible_rows);
        if max_scroll == 0 {
            return;
        }

        let (_, thumb_h) = ui::scrollbar::scrollbar_thumb(total_rows, visible_rows, 0, track_h);
        let scroll_space = track_h.saturating_sub(thumb_h);
        if scroll_space == 0 {
            return;
        }

        let row_rel = row.saturating_sub(sb_area.y) as usize;
        let thumb_top = row_rel.saturating_sub(grab_offset as usize).min(scroll_space);
        let target_offset = (thumb_top * max_scroll + (scroll_space / 2)) / scroll_space;
        let target_index = target_offset.min(self.right.entries.len().saturating_sub(1));
        self.right.selected_index = target_index;
        self.right.table_state.select(Some(target_index));
        self.active_panel = DualPanelSide::Right;
    }

    pub(crate) fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) -> Option<KeyEvent> {
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                if self.is_dual_panel_mode()
                    && let Some((left_frame, right_frame)) = self.dual_panel_frame_areas(area) {
                        let in_left = mouse.column >= left_frame.x
                            && mouse.column < left_frame.x + left_frame.width
                            && mouse.row >= left_frame.y
                            && mouse.row < left_frame.y + left_frame.height;
                        let in_right = mouse.column >= right_frame.x
                            && mouse.column < right_frame.x + right_frame.width
                            && mouse.row >= right_frame.y
                            && mouse.row < right_frame.y + right_frame.height;
                        if in_left {
                            self.active_panel = DualPanelSide::Left;
                        } else if in_right {
                            self.active_panel = DualPanelSide::Right;
                        }
                    }
                self.handle_mouse_scroll(matches!(mouse.kind, MouseEventKind::ScrollUp));
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.left.list_scroll_dragging = false;
                if matches!(
                    self.mode,
                    AppMode::DownloadInput | AppMode::DownloadNaming
                ) {
                    self.paste_clipboard_at_input_cursor();
                    return None;
                }
                if matches!(self.mode, AppMode::Browsing | AppMode::PathEditing) {
                    return Some(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.is_dual_panel_mode() {
                    if let Some((left_frame, right_frame)) = self.dual_panel_frame_areas(area) {
                        let in_left = mouse.column >= left_frame.x
                            && mouse.column < left_frame.x + left_frame.width
                            && mouse.row >= left_frame.y
                            && mouse.row < left_frame.y + left_frame.height;
                        let in_right = mouse.column >= right_frame.x
                            && mouse.column < right_frame.x + right_frame.width
                            && mouse.row >= right_frame.y
                            && mouse.row < right_frame.y + right_frame.height;
                        if in_left {
                            self.active_panel = DualPanelSide::Left;
                        } else if in_right {
                            self.active_panel = DualPanelSide::Right;
                        }
                    }

                    if let Some(sb_area) = self.right_table_scrollbar_area(area)
                        && mouse.column >= sb_area.x
                            && mouse.column < sb_area.x + sb_area.width
                            && mouse.row >= sb_area.y
                            && mouse.row < sb_area.y + sb_area.height
                        {
                            let total_rows = self.right.entries.len();
                            if let Some(grab_offset) = Self::scrollbar_grab_offset_for_row(
                                sb_area,
                                total_rows,
                                self.right.table_state.offset(),
                                mouse.row,
                            ) {
                                self.right.list_scroll_grab_offset = grab_offset;
                                self.right.list_scroll_dragging = true;
                                self.scroll_right_list_from_scrollbar_row(
                                    area,
                                    mouse.row,
                                    self.right.list_scroll_grab_offset,
                                );
                                return None;
                            }
                        }
                }

                if let Some(sb_area) = self.main_table_scrollbar_area(area)
                    && mouse.column >= sb_area.x
                        && mouse.column < sb_area.x + sb_area.width
                        && mouse.row >= sb_area.y
                        && mouse.row < sb_area.y + sb_area.height
                    {
                        let total_rows = self.left.entries.len();
                        if let Some(grab_offset) = Self::scrollbar_grab_offset_for_row(
                            sb_area,
                            total_rows,
                            self.left.table_state.offset(),
                            mouse.row,
                        ) {
                            self.left.list_scroll_grab_offset = grab_offset;
                            self.left.list_scroll_dragging = true;
                            self.scroll_main_list_from_scrollbar_row(
                                area,
                                mouse.row,
                                self.left.list_scroll_grab_offset,
                            );
                            return None;
                        }
                    }
                self.left.list_scroll_dragging = false;
                self.right.list_scroll_dragging = false;
                if self.handle_preview_pane_tab_click(mouse.column, mouse.row, area) {
                    return None;
                }
                // Footer pills use exact stored hit-zones, so resolve them before
                // the background file-list click (which would otherwise mutate the
                // selection or register a double-click) and before the overlay
                // body row hit-test.
                if let Some(key) = self.handle_footer_shortcut_click(mouse.column, mouse.row) {
                    return Some(key);
                }
                if let Some(key) = self.handle_main_list_click(mouse.column, mouse.row, area) {
                    return Some(key);
                }
                if let Some(key) = self.handle_right_list_click(mouse.column, mouse.row, area) {
                    return Some(key);
                }
                if self.handle_tab_close_click(mouse.column, mouse.row, area) {
                    return None;
                }
                if self.handle_tab_click(mouse.column, mouse.row, area) {
                    return None;
                }
                if let Some(key) = self.clickable_key_from_tabbed_row(mouse.column, mouse.row, area) {
                    return Some(key);
                }
                let _ = self.handle_confirm_delete_click(mouse.column, mouse.row, area);
                if self.handle_confirm_integration_install_click(mouse.column, mouse.row, area) {
                    return None;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.left.list_scroll_dragging {
                    self.scroll_main_list_from_scrollbar_row(
                        area,
                        mouse.row,
                        self.left.list_scroll_grab_offset,
                    );
                    return None;
                }
                if self.right.list_scroll_dragging {
                    self.scroll_right_list_from_scrollbar_row(
                        area,
                        mouse.row,
                        self.right.list_scroll_grab_offset,
                    );
                    return None;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.left.list_scroll_dragging = false;
                self.right.list_scroll_dragging = false;
            }
            _ => {}
        }

        None
    }
}
