use std::{
    collections::{HashMap, HashSet},
    env,
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
};

use crossterm::{
    cursor::{Hide, Show},
    execute,
};

use crate::util::background::{drain_channel, spawn_worker};
use crate::util::tui::{resume_tui, suspend_tui};
use crate::{App, AppMode, DualPanelSide, NotesLoadMsg};

impl App {

    pub(crate) fn notes_file_path(dir: &Path) -> PathBuf {
        dir.join(".sb")
    }

    pub(crate) fn escape_note_field(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                _ => out.push(ch),
            }
        }
        out
    }

    pub(crate) fn unescape_note_field(input: &str) -> Option<String> {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                out.push(ch);
                continue;
            }

            let esc = chars.next()?;
            match esc {
                '\\' => out.push('\\'),
                't' => out.push('\t'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                _ => return None,
            }
        }
        Some(out)
    }

    pub(crate) fn load_notes_map_for_dir(dir: &Path) -> HashMap<String, String> {
        let path = Self::notes_file_path(dir);
        let Ok(raw) = fs::read_to_string(path) else {
            return HashMap::new();
        };

        let mut notes = HashMap::new();
        for line in raw.lines() {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let Some(name_raw) = parts.next() else {
                continue;
            };
            let Some(note_raw) = parts.next() else {
                continue;
            };
            let Some(name) = Self::unescape_note_field(name_raw) else {
                continue;
            };
            let Some(note) = Self::unescape_note_field(note_raw) else {
                continue;
            };
            if name.is_empty() || note.trim().is_empty() {
                continue;
            }
            notes.insert(name, note);
        }
        notes
    }

    pub(crate) fn request_notes_for_current_dir_once(&mut self) {
        if self.notes_rx.is_some() {
            return;
        }
        if self
            .notes_loaded_for
            .as_ref()
            .map(|p| p == &self.left.dir)
            .unwrap_or(false)
        {
            return;
        }

        self.notes_scan_id = self.notes_scan_id.wrapping_add(1);
        let scan_id = self.notes_scan_id;
        let dir = self.left.dir.clone();
        self.notes_by_name.clear();
        self.notes_rx = Some(spawn_worker(move |tx| {
            let notes = App::load_notes_map_for_dir(&dir);
            let _ = tx.send(NotesLoadMsg::Finished(scan_id, dir, notes));
        }));
    }

    pub(crate) fn pump_notes_progress(&mut self) {
        for NotesLoadMsg::Finished(scan_id, path, notes) in drain_channel(&mut self.notes_rx) {
            if scan_id == self.notes_scan_id && path == self.left.dir {
                self.notes_by_name = notes;
                self.notes_loaded_for = Some(path);
            }
        }
    }

    pub(crate) fn selected_note_targets(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let is_right = self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right;
        if is_right {
            if !self.right.marked_indices.is_empty() {
                for idx in &self.right.marked_indices {
                    if let Some(entry) = self.right.entries.get(*idx) {
                        out.push(crate::util::classify::entry_name(entry));
                    }
                }
            } else if let Some(entry) = self.right.entries.get(self.right.selected_index) {
                out.push(crate::util::classify::entry_name(entry));
            }
        } else if !self.left.marked_indices.is_empty() {
            for idx in &self.left.marked_indices {
                if let Some(entry) = self.left.entries.get(*idx) {
                    out.push(crate::util::classify::entry_name(entry));
                }
            }
        } else if let Some(entry) = self.left.entries.get(self.left.selected_index) {
            out.push(crate::util::classify::entry_name(entry));
        }
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn begin_note_edit(&mut self) {
        let targets = self.selected_note_targets();
        if targets.is_empty() {
            self.set_status("no selected item");
            return;
        }

        let active_dir = self.active_panel_dir();
        let notes_map = if active_dir != self.left.dir {
            Self::load_notes_map_for_dir(&active_dir)
        } else {
            self.notes_by_name.clone()
        };

        let initial = if targets.len() == 1 {
            notes_map.get(&targets[0]).cloned().unwrap_or_default()
        } else {
            String::new()
        };

        self.note_edit_dir = active_dir;
        self.note_edit_targets = targets;
        self.begin_input_edit(AppMode::NoteEditing, initial);
    }

    pub(crate) fn entry_names_in_dir(dir: &PathBuf) -> HashSet<String> {
        let mut names = HashSet::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return names;
        };
        for entry in entries.flatten() {
            let name = crate::util::classify::entry_name(&entry);
            if name == ".sb" {
                continue;
            }
            names.insert(name);
        }
        names
    }

    pub(crate) fn write_notes_map(dir: &Path, notes: &HashMap<String, String>, scan_id: u64) -> io::Result<()> {
        let notes_path = Self::notes_file_path(dir);
        if notes.is_empty() {
            match fs::remove_file(&notes_path) {
                Ok(()) | Err(_) => {}
            }
            return Ok(());
        }
        let mut keys: Vec<&String> = notes.keys().collect();
        keys.sort();
        let mut lines: Vec<String> = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(note) = notes.get(key) {
                lines.push(format!(
                    "{}\t{}",
                    Self::escape_note_field(key),
                    Self::escape_note_field(note)
                ));
            }
        }
        let mut payload = lines.join("\n");
        payload.push('\n');
        let tmp_path = dir.join(format!(".sb.tmp.{}", scan_id));
        fs::write(&tmp_path, &payload)?;
        fs::rename(&tmp_path, &notes_path)?;
        Ok(())
    }

    pub(crate) fn save_notes_for_current_dir(&mut self) -> io::Result<()> {
        let existing = Self::entry_names_in_dir(&self.left.dir);
        self.notes_by_name
            .retain(|name, note| existing.contains(name) && !note.trim().is_empty());
        Self::write_notes_map(&self.left.dir, &self.notes_by_name, self.notes_scan_id)?;
        self.notes_loaded_for = Some(self.left.dir.clone());
        Ok(())
    }

    pub(crate) fn commit_note_edit(&mut self) {
        if self.note_edit_targets.is_empty() {
            self.clear_input_edit();
            self.mode = AppMode::Browsing;
            return;
        }

        let note = self.input_buffer.clone();
        let is_empty = note.trim().is_empty();
        let count = self.note_edit_targets.len();
        let edit_dir = self.note_edit_dir.clone();

        let save_result = if edit_dir == self.left.dir || edit_dir == PathBuf::new() {
            for target in &self.note_edit_targets {
                if is_empty {
                    self.notes_by_name.remove(target);
                } else {
                    self.notes_by_name.insert(target.clone(), note.clone());
                }
            }
            self.save_notes_for_current_dir()
        } else {
            let mut notes = Self::load_notes_map_for_dir(&edit_dir);
            let existing = Self::entry_names_in_dir(&edit_dir);
            notes.retain(|name, n| existing.contains(name) && !n.trim().is_empty());
            for target in &self.note_edit_targets {
                if is_empty {
                    notes.remove(target);
                } else {
                    notes.insert(target.clone(), note.clone());
                }
            }
            Self::write_notes_map(&edit_dir, &notes, self.notes_scan_id)
        };

        match save_result {
            Ok(()) => {
                // Keep both panels consistent: in dual-panel mode the same
                // directory can be shown in both halves, so any panel whose dir
                // matches the edited dir must pick up the just-saved note.
                let saved = Self::load_notes_map_for_dir(&edit_dir);
                if self.left.dir == edit_dir {
                    self.notes_by_name = saved.clone();
                    self.notes_loaded_for = Some(edit_dir.clone());
                }
                if self.is_dual_panel_mode() && self.right.dir == edit_dir {
                    self.right_notes_by_name = saved;
                    self.right_notes_loaded_for = Some(edit_dir.clone());
                }
                if is_empty {
                    self.set_status(format!("cleared note for {} item(s)", count));
                } else {
                    self.set_status(format!("saved note for {} item(s)", count));
                }
            }
            Err(e) => {
                self.set_status(format!("save note failed: {}", e));
            }
        }

        self.note_edit_targets.clear();
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
    }

    pub(crate) fn open_todo_file_in_editor(&mut self) -> io::Result<()> {
        let home = match env::var("HOME") {
            Ok(v) => v,
            Err(_) => {
                self.set_status("HOME is not set");
                return Ok(());
            }
        };

        let todo_path = PathBuf::from(home).join(".todo");
        if !todo_path.exists()
            && let Err(e) = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&todo_path)
            {
                self.set_status(format!("failed to create ~/.todo: {}", e));
                return Ok(());
            }

        let editor = crate::util::command::editor_command();
        suspend_tui()?;
        execute!(io::stdout(), Show)?;
        let _ = Command::new(editor).arg(&todo_path).status();
        resume_tui()?;
        execute!(io::stdout(), Hide)?;
        self.refresh_entries_or_status();
        self.set_status("opened ~/.todo");
        Ok(())
    }
}
