//! App-side plugin glue: the per-tick hook diff, the `sb.spawn` result pump,
//! and the Plugins panel state helpers.

use crossterm::event::{KeyCode, KeyEvent};

use crate::plugin::PluginMsg;
use crate::plugin::runtime::Hook;
use crate::util::keymap::{KeyCombo, KeyMap};
use crate::{App, AppMode};

impl App {
    /// Fire `on_cd`/`on_select` hooks by diffing the active panel's dir and
    /// selection against the last observed values. Called once per event-loop
    /// iteration; there is no single choke point for directory changes, so a
    /// tick-diff catches every path (bookmarks, mounts, archives, `~`, ...).
    /// The first tick only records the baseline.
    pub(crate) fn plugin_tick(&mut self) {
        if self.plugins.hooks_running {
            return;
        }
        let wants_cd = self.plugins.wants_hook(Hook::Cd);
        let wants_select = self.plugins.wants_hook(Hook::Select);
        if !wants_cd && !wants_select {
            return;
        }
        let dir = self.active_panel_dir();
        let selected = self.active_selected_entry_path();
        let prev_dir = self.plugins.last_dir.replace(dir.clone());
        let prev_selected = std::mem::replace(&mut self.plugins.last_selected, selected.clone());
        let Some(prev_dir) = prev_dir else {
            return; // baseline tick
        };

        self.plugins.hooks_running = true;
        if wants_cd && prev_dir != dir {
            let mut ctx = self.plugin_ctx();
            ctx.prev_dir = Some(prev_dir);
            let effects = self.plugins.run_hook(Hook::Cd, &ctx);
            self.apply_plugin_effects(effects, false);
        }
        if wants_select && prev_selected != selected && selected.is_some() {
            let ctx = self.plugin_ctx();
            let effects = self.plugins.run_hook(Hook::Select, &ctx);
            self.apply_plugin_effects(effects, false);
        }
        self.plugins.hooks_running = false;
        // Hook effects may have changed dir/selection again; refresh the
        // baseline so the next tick doesn't re-fire for our own changes.
        self.plugins.last_dir = Some(self.active_panel_dir());
        self.plugins.last_selected = self.active_selected_entry_path();
    }

    /// Drain finished `sb.spawn` jobs and run their Lua callbacks. Returns
    /// true when anything was processed (the event loop repaints then).
    pub(crate) fn pump_plugins(&mut self) -> bool {
        let mut had_work = false;
        while let Some(msg) = self.plugins.poll_spawn() {
            had_work = true;
            let PluginMsg::SpawnDone { token, plugin, status, stdout, stderr } = msg;
            match self
                .plugins
                .run_spawn_callback(token, &plugin, status, &stdout, &stderr)
            {
                Ok(effects) => {
                    self.apply_plugin_effects(effects, false);
                }
                Err(msg) => self.set_status(format!("plugin {}: {}", plugin, msg)),
            }
        }
        if had_work {
            self.needs_redraw = true;
        }
        had_work
    }

    /// Open the Plugins panel (tab 9).
    pub(crate) fn open_plugins_panel(&mut self) {
        self.plugins_panel.selected = 0;
        self.plugins_panel.key_capture = false;
        self.panel_tab = 9;
        self.mode = AppMode::Plugins;
    }

    /// Name of the plugin on the selected Plugins-panel row.
    pub(crate) fn selected_plugin_name(&self) -> Option<String> {
        self.plugins
            .plugins
            .get(self.plugins_panel.selected)
            .map(|p| p.source.name.clone())
    }

    /// Toggle the selected plugin on/off. Enabling (re)loads its script;
    /// disabling drops its bindings/previewers/hooks. Persisted immediately.
    pub(crate) fn toggle_selected_plugin(&mut self) {
        let idx = self.plugins_panel.selected;
        let Some(p) = self.plugins.plugins.get_mut(idx) else {
            return;
        };
        p.enabled = !p.enabled;
        let (name, enabled) = (p.source.name.clone(), p.enabled);
        if enabled {
            self.plugins.load_plugin(idx);
        }
        self.plugins.rebuild_indexes();
        let result = crate::util::config::SbPersistConfig::update(|cfg| {
            cfg.disabled_plugins.retain(|n| n != &name);
            if !enabled {
                cfg.disabled_plugins.push(name.clone());
            }
        });
        if let Err(e) = result {
            self.set_status(format!("failed to save plugin state: {}", e));
        } else {
            self.set_status(format!(
                "plugin {} {}",
                name,
                if enabled { "enabled" } else { "disabled" }
            ));
        }
    }

    /// Handle the key pressed while the Plugins panel captures a binding for
    /// the selected plugin's `entry()`. Mirrors `apply_shortcut_capture`:
    /// reserved keys and conflicts (built-in actions or other plugins) keep
    /// the capture open with an explanatory status.
    pub(crate) fn apply_plugin_key_capture(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Modifier(_) | KeyCode::Null) {
            return;
        }
        let combo = KeyCombo::from_event(key);
        let Some(name) = self.selected_plugin_name() else {
            self.plugins_panel.key_capture = false;
            return;
        };
        if KeyMap::is_reserved(combo) {
            self.set_status(format!(
                "{} is reserved — press another key (Esc cancels)",
                combo.label()
            ));
            return;
        }
        if let Some(other) = self.keymap.conflict(combo, usize::MAX) {
            self.set_status(format!(
                "{} is already used by \"{}\" — press another key (Esc cancels)",
                combo.label(),
                other.label
            ));
            return;
        }
        if let Some(other) = self
            .plugins
            .plugins
            .iter()
            .find(|p| p.source.name != name && p.bound_key == Some(combo))
        {
            self.set_status(format!(
                "{} is already used by plugin \"{}\" — press another key (Esc cancels)",
                combo.label(),
                other.source.name
            ));
            return;
        }
        self.plugins_panel.key_capture = false;
        self.set_plugin_binding(&name, Some(combo));
        self.set_status(format!("{} bound to {}", name, combo.label()));
    }

    /// Remove the selected plugin's key binding.
    pub(crate) fn reset_selected_plugin_key(&mut self) {
        let Some(name) = self.selected_plugin_name() else {
            return;
        };
        if self
            .plugins
            .plugins
            .get(self.plugins_panel.selected)
            .and_then(|p| p.bound_key)
            .is_none()
        {
            return;
        }
        self.set_plugin_binding(&name, None);
        self.set_status(format!("{} key binding removed", name));
    }

    /// Build the display rows for the Plugins panel.
    pub(crate) fn plugin_panel_rows(&self) -> Vec<crate::ui::panels::PluginRow> {
        self.plugins
            .plugins
            .iter()
            .map(|p| {
                let (state, detail) = if let Some(err) = &p.last_error {
                    ("error", err.clone())
                } else if !p.enabled {
                    ("off", "disabled (Space to enable)".to_string())
                } else {
                    let mut caps = Vec::new();
                    if p.has_entry {
                        caps.push("command".to_string());
                    }
                    if !p.preview_exts.is_empty() {
                        caps.push(format!("previewer ({})", p.preview_exts.join(",")));
                    }
                    let mut hooks = Vec::new();
                    for (on, name) in [
                        (p.hooks.start, "start"),
                        (p.hooks.cd, "cd"),
                        (p.hooks.select, "select"),
                        (p.hooks.quit, "quit"),
                    ] {
                        if on {
                            hooks.push(name);
                        }
                    }
                    if !hooks.is_empty() {
                        caps.push(format!("hooks ({})", hooks.join(",")));
                    }
                    if caps.is_empty() {
                        caps.push("no capabilities".to_string());
                    }
                    ("active", caps.join(", "))
                };
                crate::ui::panels::PluginRow {
                    name: p.source.name.clone(),
                    key: p.bound_key.map(|c| c.label()),
                    state,
                    detail,
                }
            })
            .collect()
    }

    /// Apply and persist a plugin key binding (removal stored as absence).
    fn set_plugin_binding(&mut self, name: &str, combo: Option<KeyCombo>) {
        if let Some(p) = self
            .plugins
            .plugins
            .iter_mut()
            .find(|p| p.source.name == name)
        {
            p.bound_key = combo;
        }
        self.plugins.rebuild_indexes();
        let name = name.to_string();
        let result = crate::util::config::SbPersistConfig::update(|cfg| match combo {
            Some(c) => {
                cfg.plugin_bindings.insert(name.clone(), c.to_config_string());
            }
            None => {
                cfg.plugin_bindings.remove(&name);
            }
        });
        if let Err(e) = result {
            self.set_status(format!("failed to save plugin bindings: {}", e));
        }
    }
}
