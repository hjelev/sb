//! Active-theme switching, render-cache rebuilds, and status-message helpers.
//! Extracted from main.rs (impl App).

use crate::ui;
use crate::{App, DualPanelSide, EntryRenderConfig};

impl App {
    pub(crate) fn set_active_theme(&mut self, theme_id: ui::theme::ThemeId) {
        self.active_theme = theme_id;
        self.theme_selected = ui::theme::themes()
            .iter()
            .position(|theme| theme.id == theme_id)
            .unwrap_or(0);
        self.os_icon = ui::icons::os_nerd_icon().map(|(glyph, _)| {
            (glyph, ui::theme::theme_spec(theme_id).icon_os)
        });
        self.rebuild_render_caches();
    }

    fn rebuild_render_caches(&mut self) {
        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
        };
        let uid_cache = App::build_uid_cache(&self.entries);
        let gid_cache = App::build_gid_cache(&self.entries);
        self.entry_render_cache = self.entries
            .iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        if !self.right.entries.is_empty() {
            let uid_cache_r = App::build_uid_cache(&self.right.entries);
            let gid_cache_r = App::build_gid_cache(&self.right.entries);
            self.right.entry_render_cache = self.right.entries
                .iter()
                .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache_r, &gid_cache_r))
                .collect();
        }
    }

    pub(crate) fn apply_selected_theme(&mut self) {
        if let Some(theme) = ui::theme::themes().get(self.theme_selected) {
            self.set_active_theme(theme.id);
        }
    }

    pub(crate) fn set_status(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right_status_message = msg;
        } else {
            self.status_message = msg;
        }
    }

    /// Set a status message reporting that a required external tool is missing.
    pub(crate) fn status_tool_not_found(&mut self, tool: &str) {
        self.set_status(format!("{} not found in PATH", tool));
    }

    pub(crate) fn panel_status_message(&self, side: DualPanelSide) -> Option<&str> {
        let msg = match side {
            DualPanelSide::Left => self.status_message.as_str(),
            DualPanelSide::Right => self.right_status_message.as_str(),
        };

        if msg.is_empty() {
            None
        } else {
            Some(msg)
        }
    }

    pub(crate) fn decorate_footer_message(&self, msg: &str) -> String {
        ui::status::decorate_footer_message(msg, self.nerd_font_active)
    }
}
