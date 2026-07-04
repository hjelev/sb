use crossterm::event::{KeyCode, KeyEvent};

use crate::util::keymap::{KeyCombo, KeyMap, ACTIONS};
use crate::App;

impl App {
    /// Handle the key pressed while the Shortcuts panel is capturing a new
    /// binding for the selected action. Reserved keys and conflicts keep the
    /// capture open with an explanatory status; a valid key is applied to the
    /// in-memory keymap and persisted immediately.
    pub(crate) fn apply_shortcut_capture(&mut self, key: KeyEvent) {
        // Ignore events that aren't real key presses (modifier/media keys).
        if matches!(key.code, KeyCode::Modifier(_) | KeyCode::Null) {
            return;
        }
        let combo = KeyCombo::from_event(key);
        let idx = self.shortcuts_selected;
        let Some(spec) = ACTIONS.get(idx) else {
            self.shortcut_capture = false;
            return;
        };
        if KeyMap::is_reserved(combo) {
            self.set_status(format!("{} is reserved — press another key (Esc cancels)", combo.label()));
            return;
        }
        if let Some(other) = self.keymap.conflict(combo, idx) {
            self.set_status(format!(
                "{} is already used by \"{}\" — press another key (Esc cancels)",
                combo.label(),
                other.label
            ));
            return;
        }
        self.shortcut_capture = false;
        self.rebind_shortcut(idx, combo);
        self.set_status(format!("{} rebound to {}", spec.label, combo.label()));
    }

    /// Reset the selected action to its default key (removing the persisted
    /// override), unless the default is now taken by another action.
    pub(crate) fn reset_selected_shortcut(&mut self) {
        let idx = self.shortcuts_selected;
        let Some(spec) = ACTIONS.get(idx) else {
            return;
        };
        if !self.keymap.is_custom_at(idx) {
            return;
        }
        if let Some(other) = self.keymap.conflict(spec.default, idx) {
            self.set_status(format!(
                "default {} is used by \"{}\" — rebind that first",
                spec.default.label(),
                other.label
            ));
            return;
        }
        self.rebind_shortcut(idx, spec.default);
        self.set_status(format!("{} reset to {}", spec.label, spec.default.label()));
    }

    /// Apply a new combo to the in-memory keymap and persist it (default
    /// bindings are stored as absence).
    fn rebind_shortcut(&mut self, idx: usize, combo: KeyCombo) {
        self.keymap.set_at(idx, combo);
        let spec = &ACTIONS[idx];
        let result = crate::util::config::SbPersistConfig::update(|cfg| {
            if combo == spec.default {
                cfg.shortcuts.remove(spec.id);
            } else {
                cfg.shortcuts.insert(spec.id.to_string(), combo.to_config_string());
            }
        });
        if let Err(e) = result {
            self.set_status(format!("failed to save shortcuts: {}", e));
        }
    }
}
