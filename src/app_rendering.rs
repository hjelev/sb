//! Active-theme switching, render-cache rebuilds, and status-message helpers.
//! Extracted from main.rs (impl App).

use crate::ui;
use crate::{App, DualPanelSide, EntryRenderConfig};

/// Which cached state a changed display setting needs to invalidate before it is
/// persisted. Used by [`App::apply_setting_change`].
enum SettingInvalidation {
    /// Rebuild the file-list render caches (icons, colors, columns).
    RenderCaches,
    /// Refresh the cached free-disk-space figures shown in the header pill.
    FreeSpace,
    /// Rebuild render caches and drop memoized previews (colors are baked in).
    RenderCachesAndPreview,
}

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
            filename_color_mode: self.filename_color_mode,
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

    /// Refresh whatever cached state a just-changed display setting affects, then
    /// persist the change. Centralizes the invalidate→persist contract shared by
    /// every Themes-panel toggle so each toggle only states *what* it changed.
    fn apply_setting_change(
        &mut self,
        invalidate: SettingInvalidation,
        persist: impl FnOnce(&mut crate::util::config::SbPersistConfig),
    ) {
        match invalidate {
            SettingInvalidation::RenderCaches => self.rebuild_render_caches(),
            SettingInvalidation::FreeSpace => self.refresh_current_dir_free_space(),
            SettingInvalidation::RenderCachesAndPreview => {
                self.rebuild_render_caches();
                // Folder previews bake the colors into their cached lines, so drop
                // the memoized previews and rebuild the current one (no-op unless
                // visible).
                self.preview_cache.clear();
                self.preview_target_path = None;
                self.request_preview_for_selected();
            }
        }
        crate::util::config::SbPersistConfig::update(persist);
    }

    /// Flip Nerd Font glyph mode, re-render the file list, and persist the
    /// choice to `~/.config/sb/config` (overriding the env var on next launch).
    pub(crate) fn toggle_nerd_font(&mut self) {
        self.nerd_font_active = !self.nerd_font_active;
        let v = self.nerd_font_active;
        self.apply_setting_change(SettingInvalidation::RenderCaches, move |cfg| {
            cfg.nerd_font = Some(v)
        });
    }

    /// Flip the "disable clock" setting and persist it. When enabled, the
    /// top-right header shows the disk-usage pill instead of the clock; refresh
    /// the disk space so the pill has data to show immediately.
    pub(crate) fn toggle_disable_clock(&mut self) {
        self.disable_clock = !self.disable_clock;
        let v = self.disable_clock;
        self.apply_setting_change(SettingInvalidation::FreeSpace, move |cfg| {
            cfg.disable_clock = Some(v)
        });
    }

    /// Cycle the filename-color mode (Full → Less → White), re-render the file
    /// list, and persist the choice to `~/.config/sb/config`.
    pub(crate) fn cycle_filename_color_mode(&mut self) {
        self.filename_color_mode = self.filename_color_mode.next();
        let v = self.filename_color_mode;
        self.apply_setting_change(SettingInvalidation::RenderCachesAndPreview, move |cfg| {
            cfg.filename_color_mode = v
        });
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
