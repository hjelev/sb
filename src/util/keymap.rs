//! Rebindable keyboard shortcuts for the Browsing mode.
//!
//! This module is the single source of truth for every rebindable command:
//! the [`ACTIONS`] table pairs each [`Action`] with its stable config id,
//! description, Help-panel category, and default [`KeyCombo`]. A [`KeyMap`]
//! overlays user overrides (persisted as `shortcut_<id> = <combo>` lines in
//! `~/.config/sb/config`) on top of those defaults and resolves incoming key
//! events to actions.
//!
//! Structural keys (arrows, Enter, Esc, Tab, Space, PgUp/PgDn, Home/End,
//! digit bookmarks, and the fixed F-key/Delete alternates) are not rebindable
//! and stay hardwired in the dispatch; [`KeyMap::is_reserved`] blocks
//! assigning them.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

/// A normalized key press: for `Char` codes the character's case carries
/// shift (`G`, never `shift+g`), and only Ctrl/Alt are kept as modifiers.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyCombo {
    pub const fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code, mods }
    }

    pub const fn char(c: char) -> Self {
        Self::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    pub const fn ctrl(c: char) -> Self {
        Self::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// Normalize a crossterm key event: keep only Ctrl/Alt (shift is already
    /// encoded in the char's case for `Char` codes).
    pub fn from_event(key: KeyEvent) -> Self {
        let mods = key.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        Self::new(key.code, mods)
    }

    /// Parse a config value like `c`, `G`, `ctrl+s`, `alt+x`, `f6`, `ctrl++`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        // Split off the key name after the last '+', treating a trailing '+'
        // as the literal '+' key ("+" and "ctrl++").
        let (mods_str, key_str) = match s.rfind('+') {
            Some(i) if i + 1 < s.len() => (&s[..i], &s[i + 1..]),
            Some(0) => ("", "+"),
            Some(i) => (&s[..i - 1], "+"),
            None => ("", s),
        };
        let mut mods = KeyModifiers::NONE;
        for tok in mods_str.split('+').filter(|t| !t.is_empty()) {
            match tok.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
                "alt" => mods |= KeyModifiers::ALT,
                // Shift is carried by the char's case; accept and drop it.
                "shift" => {}
                _ => return None,
            }
        }
        let lower = key_str.to_ascii_lowercase();
        let code = match lower.as_str() {
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "enter" | "return" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pgup" | "pageup" => KeyCode::PageUp,
            "pgdn" | "pagedown" => KeyCode::PageDown,
            "backspace" | "bksp" => KeyCode::Backspace,
            "del" | "delete" => KeyCode::Delete,
            "ins" | "insert" => KeyCode::Insert,
            k if k.len() >= 2 && k.starts_with('f') && k[1..].chars().all(|c| c.is_ascii_digit()) => {
                let n: u8 = k[1..].parse().ok()?;
                if !(1..=12).contains(&n) {
                    return None;
                }
                KeyCode::F(n)
            }
            _ => {
                let mut chars = key_str.chars();
                let c = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                KeyCode::Char(c)
            }
        };
        Some(Self::new(code, mods))
    }

    fn key_name(&self) -> String {
        match self.code {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("f{}", n),
            KeyCode::Tab => "tab".to_string(),
            KeyCode::BackTab => "backtab".to_string(),
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Esc => "esc".to_string(),
            KeyCode::Up => "up".to_string(),
            KeyCode::Down => "down".to_string(),
            KeyCode::Left => "left".to_string(),
            KeyCode::Right => "right".to_string(),
            KeyCode::Home => "home".to_string(),
            KeyCode::End => "end".to_string(),
            KeyCode::PageUp => "pgup".to_string(),
            KeyCode::PageDown => "pgdn".to_string(),
            KeyCode::Backspace => "backspace".to_string(),
            KeyCode::Delete => "del".to_string(),
            KeyCode::Insert => "ins".to_string(),
            _ => "?".to_string(),
        }
    }

    /// The config-file form (`ctrl+s`, `G`, `f6`).
    pub fn to_config_string(&self) -> String {
        let mut s = String::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            s.push_str("ctrl+");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            s.push_str("alt+");
        }
        s.push_str(&self.key_name());
        s
    }

    /// The human-readable form shown in the UI (`Ctrl+s`, `G`, `F6`).
    pub fn label(&self) -> String {
        let key = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("F{}", n),
            KeyCode::Delete => "Del".to_string(),
            KeyCode::Insert => "Ins".to_string(),
            _ => {
                let name = self.key_name();
                let mut chars = name.chars();
                match chars.next() {
                    Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                    None => name,
                }
            }
        };
        let mut s = String::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            s.push_str("Ctrl+");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            s.push_str("Alt+");
        }
        s.push_str(&key);
        s
    }
}

/// Every rebindable Browsing-mode command.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    FolderFilter,
    TogglePreview,
    ToggleFolderSizes,
    SortMenu,
    ToggleHidden,
    GoHome,
    TreeExpand,
    TreeCollapse,
    MarkAll,
    NoteEdit,
    CopyPaths,
    EditClipboard,
    NewFile,
    Copy,
    Paste,
    Move,
    Rename,
    Edit,
    Delete,
    ToggleExec,
    Zip,
    OpenDefault,
    AgeProtect,
    ViewFile,
    Download,
    FzfFind,
    Grep,
    RemoteMounts,
    DeltaCompare,
    SplitLess,
    SplitEditor,
    Integrations,
    Bookmarks,
    Themes,
    Plugins,
    GitCommit,
    GitLog,
    DropShell,
    TodoFile,
    CommandInput,
    Organize,
    Help,
    Quit,
}

/// One entry of the static action table: identity, description, category
/// (matches the Help panel sections), the rebindable default key, and an
/// optional fixed (non-rebindable) alternate shown for reference.
pub struct ActionSpec {
    pub action: Action,
    /// Stable config id: persisted as `shortcut_<id> = <combo>`.
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub default: KeyCombo,
    /// Display-only fixed alternate key (stays hardwired in the dispatch).
    pub fixed_alt: Option<&'static str>,
}

const NAV: &str = "Navigation & View";
const SEL: &str = "Selection & Metadata";
const OPS: &str = "File Operations";
const EXT: &str = "Search & External";
const SYS: &str = "System & Git";

pub static ACTIONS: &[ActionSpec] = &[
    ActionSpec { action: Action::FolderFilter, id: "folder_filter", label: "Filter folder by name/regex", category: NAV, default: KeyCombo::char('/'), fixed_alt: None },
    ActionSpec { action: Action::TogglePreview, id: "toggle_preview", label: "Toggle preview mode", category: NAV, default: KeyCombo::char('`'), fixed_alt: None },
    ActionSpec { action: Action::ToggleFolderSizes, id: "toggle_folder_sizes", label: "Toggle folder size calc", category: NAV, default: KeyCombo::char('s'), fixed_alt: None },
    ActionSpec { action: Action::SortMenu, id: "sort_menu", label: "Open sorting menu", category: NAV, default: KeyCombo::ctrl('s'), fixed_alt: None },
    ActionSpec { action: Action::ToggleHidden, id: "toggle_hidden", label: "Toggle hidden files", category: NAV, default: KeyCombo::char('.'), fixed_alt: None },
    ActionSpec { action: Action::GoHome, id: "go_home", label: "Go to home directory", category: NAV, default: KeyCombo::char('~'), fixed_alt: None },
    ActionSpec { action: Action::TreeExpand, id: "tree_expand", label: "Expand tree on selected dirs", category: NAV, default: KeyCombo::char('+'), fixed_alt: None },
    ActionSpec { action: Action::TreeCollapse, id: "tree_collapse", label: "Collapse tree", category: NAV, default: KeyCombo::char('-'), fixed_alt: None },
    ActionSpec { action: Action::MarkAll, id: "mark_all", label: "Toggle all marks in directory", category: SEL, default: KeyCombo::char('*'), fixed_alt: None },
    ActionSpec { action: Action::NoteEdit, id: "note_edit", label: "Add/edit note for selected item", category: SEL, default: KeyCombo::ctrl('n'), fixed_alt: None },
    ActionSpec { action: Action::CopyPaths, id: "copy_paths", label: "Copy full path(s) to clipboard", category: SEL, default: KeyCombo::ctrl('c'), fixed_alt: None },
    ActionSpec { action: Action::EditClipboard, id: "edit_clipboard", label: "Edit system clipboard", category: SEL, default: KeyCombo::ctrl('e'), fixed_alt: None },
    ActionSpec { action: Action::NewFile, id: "new_file", label: "New 'file' or '/folder'", category: OPS, default: KeyCombo::char('n'), fixed_alt: None },
    ActionSpec { action: Action::Copy, id: "copy", label: "Copy marked to app clipboard", category: OPS, default: KeyCombo::char('c'), fixed_alt: Some("F5") },
    ActionSpec { action: Action::Paste, id: "paste", label: "Paste clipboard to folder", category: OPS, default: KeyCombo::char('v'), fixed_alt: None },
    ActionSpec { action: Action::Move, id: "move", label: "Move clipboard to folder", category: OPS, default: KeyCombo::char('m'), fixed_alt: None },
    ActionSpec { action: Action::Rename, id: "rename", label: "Rename or bulk rename", category: OPS, default: KeyCombo::char('r'), fixed_alt: Some("F2") },
    ActionSpec { action: Action::Edit, id: "edit", label: "Edit file (or rename folder)", category: OPS, default: KeyCombo::char('e'), fixed_alt: Some("F4") },
    ActionSpec { action: Action::Delete, id: "delete", label: "Delete selected item(s)", category: OPS, default: KeyCombo::char('d'), fixed_alt: Some("Del") },
    ActionSpec { action: Action::ToggleExec, id: "toggle_exec", label: "Toggle executable flag", category: OPS, default: KeyCombo::char('x'), fixed_alt: None },
    ActionSpec { action: Action::Zip, id: "zip", label: "Create or extract archive", category: OPS, default: KeyCombo::char('Z'), fixed_alt: None },
    ActionSpec { action: Action::OpenDefault, id: "open_default", label: "Open with default GUI app", category: OPS, default: KeyCombo::char('o'), fixed_alt: None },
    ActionSpec { action: Action::AgeProtect, id: "age_protect", label: "Protect file with age", category: OPS, default: KeyCombo::char('p'), fixed_alt: None },
    ActionSpec { action: Action::ViewFile, id: "view_file", label: "View file in pager", category: OPS, default: KeyCombo::char('l'), fixed_alt: None },
    ActionSpec { action: Action::Download, id: "download", label: "Download URL", category: OPS, default: KeyCombo::char('w'), fixed_alt: None },
    ActionSpec { action: Action::FzfFind, id: "fzf_find", label: "Fuzzy file search", category: EXT, default: KeyCombo::char('f'), fixed_alt: None },
    ActionSpec { action: Action::Grep, id: "grep", label: "Content search", category: EXT, default: KeyCombo::char('g'), fixed_alt: None },
    ActionSpec { action: Action::RemoteMounts, id: "remote_mounts", label: "Open SSH/rclone mount picker", category: EXT, default: KeyCombo::char('S'), fixed_alt: None },
    ActionSpec { action: Action::DeltaCompare, id: "delta_compare", label: "Delta compare (marked vs cursor)", category: EXT, default: KeyCombo::char('C'), fixed_alt: None },
    ActionSpec { action: Action::SplitLess, id: "split_less", label: "Split shell + preview", category: EXT, default: KeyCombo::char('i'), fixed_alt: None },
    ActionSpec { action: Action::SplitEditor, id: "split_editor", label: "Split shell + edit", category: EXT, default: KeyCombo::char('E'), fixed_alt: None },
    ActionSpec { action: Action::Integrations, id: "integrations", label: "Open integrations panel", category: EXT, default: KeyCombo::char('I'), fixed_alt: None },
    ActionSpec { action: Action::Bookmarks, id: "bookmarks", label: "Open bookmarks", category: EXT, default: KeyCombo::char('b'), fixed_alt: None },
    ActionSpec { action: Action::Themes, id: "themes", label: "Open themes panel", category: EXT, default: KeyCombo::char('T'), fixed_alt: None },
    ActionSpec { action: Action::Plugins, id: "plugins", label: "Open plugins panel", category: EXT, default: KeyCombo::char('P'), fixed_alt: None },
    ActionSpec { action: Action::GitCommit, id: "git_commit", label: "Git: commit + push", category: SYS, default: KeyCombo::char('G'), fixed_alt: None },
    ActionSpec { action: Action::GitLog, id: "git_log", label: "Git: view pretty log graph", category: SYS, default: KeyCombo::char('H'), fixed_alt: None },
    ActionSpec { action: Action::DropShell, id: "drop_shell", label: "Drop to shell in current dir", category: SYS, default: KeyCombo::ctrl('z'), fixed_alt: None },
    ActionSpec { action: Action::TodoFile, id: "todo_file", label: "Open ~/.todo in $EDITOR", category: SYS, default: KeyCombo::char('t'), fixed_alt: None },
    ActionSpec { action: Action::CommandInput, id: "command_input", label: "Run shell command", category: SYS, default: KeyCombo::char(';'), fixed_alt: None },
    ActionSpec { action: Action::Organize, id: "organize", label: "AI: organize current folder", category: SYS, default: KeyCombo::ctrl('o'), fixed_alt: None },
    ActionSpec { action: Action::Help, id: "help", label: "Open help screen", category: SYS, default: KeyCombo::char('h'), fixed_alt: None },
    ActionSpec { action: Action::Quit, id: "quit", label: "Quit Shell Buddy", category: SYS, default: KeyCombo::char('q'), fixed_alt: Some("Esc") },
];

fn action_index(action: Action) -> usize {
    ACTIONS
        .iter()
        .position(|spec| spec.action == action)
        .expect("every Action has an ACTIONS entry")
}

/// The active key bindings: defaults overlaid with the user's persisted
/// overrides. `combos` is parallel to [`ACTIONS`].
pub struct KeyMap {
    combos: Vec<KeyCombo>,
    by_key: HashMap<KeyCombo, usize>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::from_overrides(&HashMap::new())
    }
}

impl KeyMap {
    /// Build from persisted overrides (`action id → combo string`). Unknown
    /// ids, unparsable combos, reserved keys, and colliding overrides fall
    /// back to the action's default.
    pub fn from_overrides(overrides: &HashMap<String, String>) -> Self {
        let mut combos: Vec<KeyCombo> = ACTIONS.iter().map(|spec| spec.default).collect();
        for (idx, spec) in ACTIONS.iter().enumerate() {
            if let Some(combo) = overrides.get(spec.id).and_then(|s| KeyCombo::parse(s))
                && !Self::is_reserved(combo)
            {
                combos[idx] = combo;
            }
        }
        // Resolve collisions deterministically: the first action keeps the
        // combo, later ones revert to their default (skipped if that is also
        // taken — the action is then unreachable until rebound).
        let mut by_key: HashMap<KeyCombo, usize> = HashMap::new();
        for (idx, spec) in ACTIONS.iter().enumerate() {
            if by_key.contains_key(&combos[idx]) {
                combos[idx] = spec.default;
            }
            by_key.entry(combos[idx]).or_insert(idx);
        }
        Self { combos, by_key }
    }

    /// Resolve a key event to its bound action, if any.
    pub fn resolve(&self, key: KeyEvent) -> Option<Action> {
        let combo = KeyCombo::from_event(key);
        self.by_key.get(&combo).map(|&idx| ACTIONS[idx].action)
    }

    /// The current combo bound to `action`.
    pub fn combo_for(&self, action: Action) -> KeyCombo {
        self.combos[action_index(action)]
    }

    /// The current combo for the action at `idx` in [`ACTIONS`].
    pub fn combo_at(&self, idx: usize) -> KeyCombo {
        self.combos[idx]
    }

    /// UI label of the current combo for `action` (e.g. `Ctrl+s`).
    pub fn label_for(&self, action: Action) -> String {
        self.combo_for(action).label()
    }

    pub fn is_custom_at(&self, idx: usize) -> bool {
        self.combos[idx] != ACTIONS[idx].default
    }

    /// The action (other than the one at `for_idx`) already bound to `combo`.
    pub fn conflict(&self, combo: KeyCombo, for_idx: usize) -> Option<&'static ActionSpec> {
        self.by_key
            .get(&combo)
            .filter(|&&idx| idx != for_idx)
            .map(|&idx| &ACTIONS[idx])
    }

    /// Rebind the action at `idx` to `combo` (in-memory only; the caller
    /// persists via `SbPersistConfig`).
    pub fn set_at(&mut self, idx: usize, combo: KeyCombo) {
        self.combos[idx] = combo;
        self.by_key.clear();
        for (i, &c) in self.combos.iter().enumerate() {
            self.by_key.entry(c).or_insert(i);
        }
    }

    /// Structural keys that must stay hardwired: navigation, dialogs, digit
    /// bookmarks, and the fixed alternates (F2/F4/F5, Del, Esc, Ctrl+g).
    pub fn is_reserved(combo: KeyCombo) -> bool {
        match combo.code {
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Enter
            | KeyCode::Esc
            | KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Insert
            | KeyCode::F(2)
            | KeyCode::F(4)
            | KeyCode::F(5) => true,
            KeyCode::Char(' ') => true,
            KeyCode::Char('0'..='9') if combo.mods.is_empty() => true,
            KeyCode::Char('g') if combo.mods == KeyModifiers::CONTROL => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        for s in ["c", "G", "ctrl+s", "alt+x", "f6", "+", "ctrl++", "space", "~", ";"] {
            let combo = KeyCombo::parse(s).unwrap_or_else(|| panic!("parse {}", s));
            assert_eq!(combo.to_config_string(), s, "roundtrip {}", s);
        }
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(KeyCombo::parse("").is_none());
        assert!(KeyCombo::parse("hyper+x").is_none());
        assert!(KeyCombo::parse("abc").is_none());
        assert!(KeyCombo::parse("f99").is_none());
    }

    #[test]
    fn from_event_strips_shift() {
        let ev = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(KeyCombo::from_event(ev), KeyCombo::char('G'));
        let ev = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(KeyCombo::from_event(ev), KeyCombo::ctrl('s'));
    }

    #[test]
    fn defaults_are_unique_and_unreserved() {
        let mut seen = HashMap::new();
        for spec in ACTIONS {
            assert!(
                !KeyMap::is_reserved(spec.default),
                "{} default is reserved",
                spec.id
            );
            if let Some(other) = seen.insert(spec.default, spec.id) {
                panic!("{} and {} share a default key", spec.id, other);
            }
        }
    }

    #[test]
    fn overrides_and_conflicts() {
        let mut overrides = HashMap::new();
        overrides.insert("rename".to_string(), "u".to_string());
        let map = KeyMap::from_overrides(&overrides);
        assert_eq!(map.combo_for(Action::Rename), KeyCombo::char('u'));
        assert_eq!(
            map.resolve(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE)),
            Some(Action::Rename)
        );
        // The old default no longer resolves.
        assert_eq!(
            map.resolve(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
            None
        );
        // Conflict detection: 'm' belongs to Move.
        let rename_idx = ACTIONS.iter().position(|s| s.action == Action::Rename).unwrap();
        let conflict = map.conflict(KeyCombo::char('m'), rename_idx).unwrap();
        assert_eq!(conflict.action, Action::Move);
        assert!(map.conflict(KeyCombo::char('u'), rename_idx).is_none());
    }

    #[test]
    fn reserved_override_falls_back_to_default() {
        let mut overrides = HashMap::new();
        overrides.insert("quit".to_string(), "enter".to_string());
        let map = KeyMap::from_overrides(&overrides);
        assert_eq!(map.combo_for(Action::Quit), KeyCombo::char('q'));
    }

    #[test]
    fn colliding_overrides_keep_first() {
        let mut overrides = HashMap::new();
        // Both claim 'y'; the earlier ACTIONS entry (copy) wins.
        overrides.insert("copy".to_string(), "y".to_string());
        overrides.insert("paste".to_string(), "y".to_string());
        let map = KeyMap::from_overrides(&overrides);
        assert_eq!(map.combo_for(Action::Copy), KeyCombo::char('y'));
        assert_eq!(map.combo_for(Action::Paste), KeyCombo::char('v'));
    }
}
