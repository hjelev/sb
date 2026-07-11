//! Applying queued [`PluginEffect`]s to the live `App`.
//!
//! Non-terminal effects are applied here in push order. Effects that suspend
//! the TUI (`EditPath`/`ViewPath`/`RunShellWait`) are returned to the caller,
//! because only the key-dispatch layer holds the terminal handle (see
//! `run_plugin_entry` in `run/key_dispatch/browsing.rs`).

use std::path::PathBuf;

use crate::{App, DualPanelSide};

use super::{PluginCtx, PluginEffect};

impl App {
    /// Snapshot the state handed to plugin `entry()` calls and hooks.
    pub(crate) fn plugin_ctx(&self) -> PluginCtx {
        let cwd = self.active_panel_dir();
        let selected = self.active_selected_entry_path();
        let on_right = self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right;
        let panel = if on_right { (&self.right, "right") } else { (&self.left, "left") };
        let mut files: Vec<PathBuf> = panel
            .0
            .entries
            .iter()
            .enumerate()
            .filter(|(i, _)| panel.0.marked_indices.contains(i))
            .map(|(_, e)| e.path())
            .collect();
        if files.is_empty()
            && let Some(sel) = &selected
        {
            files.push(sel.clone());
        }
        PluginCtx {
            cwd,
            selected,
            files,
            panel: panel.1,
            prev_dir: None,
        }
    }

    /// Apply the non-terminal effects in order; returns the TUI-suspending
    /// ones (in order) for the dispatch layer to run, or drops them with a
    /// warning when `allow_terminal` is false (hooks must not suspend the TUI).
    pub(crate) fn apply_plugin_effects(
        &mut self,
        effects: Vec<PluginEffect>,
        allow_terminal: bool,
    ) -> Vec<PluginEffect> {
        let mut terminal_effects = Vec::new();
        for effect in effects {
            if effect.needs_terminal() {
                if allow_terminal {
                    terminal_effects.push(effect);
                } else {
                    self.set_status("plugin: TUI-suspending calls are ignored in hooks");
                }
                continue;
            }
            self.apply_one(effect);
        }
        self.needs_redraw = true;
        terminal_effects
    }

    fn apply_one(&mut self, effect: PluginEffect) {
        match effect {
            PluginEffect::Status(msg) => self.set_status(msg),
            PluginEffect::Cd(path) => {
                let target = self.resolve_plugin_path(path);
                if target.is_dir() {
                    self.try_enter_dir_on_active_panel(target);
                } else {
                    self.set_status(format!("plugin cd: not a directory: {}", target.display()));
                }
            }
            PluginEffect::RefreshDir => {
                if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
                    if self.refresh_right_panel_entries().is_err() {
                        self.set_status("refresh failed");
                    }
                } else {
                    self.refresh_entries_or_status();
                }
            }
            PluginEffect::SelectName(name) => {
                if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
                    self.select_right_entry_named(&name);
                } else {
                    self.select_entry_named(&name);
                }
            }
            PluginEffect::MarkNames(names) => {
                let on_right =
                    self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right;
                let panel = if on_right { &mut self.right } else { &mut self.left };
                for name in names {
                    if let Some(index) = panel
                        .entries
                        .iter()
                        .position(|e| e.file_name().to_string_lossy() == name)
                    {
                        panel.marked_indices.insert(index);
                    }
                }
                self.start_selected_total_size_scan();
            }
            PluginEffect::ClearMarks => {
                let on_right =
                    self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right;
                let panel = if on_right { &mut self.right } else { &mut self.left };
                panel.marked_indices.clear();
            }
            PluginEffect::ClipboardSet(paths) => {
                let count = paths.len();
                let resolved: Vec<PathBuf> = paths
                    .into_iter()
                    .map(|p| self.resolve_plugin_path(p))
                    .collect();
                self.clipboard = resolved;
                self.set_status(format!("plugin: {} item(s) in clipboard", count));
            }
            // Terminal effects are filtered out by apply_plugin_effects.
            PluginEffect::EditPath(_) | PluginEffect::ViewPath(_) | PluginEffect::RunShellWait(_) => {}
        }
    }

    /// Plugin-supplied paths may be relative; resolve them against the active
    /// panel's directory.
    pub(crate) fn resolve_plugin_path(&self, path: PathBuf) -> PathBuf {
        if path.is_absolute() {
            path
        } else {
            self.active_panel_dir().join(path)
        }
    }
}
