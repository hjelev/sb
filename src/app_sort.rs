//! Entry sorting and the sort-menu flow. Extracted from main.rs (impl App).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::{App, AppMode, EntryRenderConfig, SortMode};

impl App {
    pub(crate) fn sort_mode_options() -> [SortMode; 7] {
        [
            SortMode::NameAsc,
            SortMode::NameDesc,
            SortMode::ExtensionAsc,
            SortMode::SizeAsc,
            SortMode::SizeDesc,
            SortMode::ModifiedNewest,
            SortMode::ModifiedOldest,
        ]
    }

    fn sort_mode_index(mode: SortMode) -> usize {
        Self::sort_mode_options()
            .iter()
            .position(|m| *m == mode)
            .unwrap_or(0)
    }

    fn entry_name_key(entry: &fs::DirEntry) -> String {
        entry.file_name().to_string_lossy().to_ascii_lowercase()
    }

    fn entry_extension_key(entry: &fs::DirEntry) -> String {
        entry.path()
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    }

    pub(crate) fn sort_entries_by_mode(
        entries: &mut Vec<fs::DirEntry>,
        mode: SortMode,
        folder_size_cache: Option<&HashMap<PathBuf, u64>>,
    ) {
        if entries.len() < 2 {
            return;
        }
        // Pre-collect all sort keys in O(n) — eliminates O(n log n) stat() calls that
        // the previous sort_by comparator incurred by calling is_file()/metadata() per pair.
        let metas: Vec<Option<fs::Metadata>> = entries.iter().map(|e| e.metadata().ok()).collect();
        let is_dirs: Vec<bool> = metas.iter()
            .map(|m| m.as_ref().map(|m| m.is_dir()).unwrap_or(false))
            .collect();
        let names: Vec<String> = entries.iter().map(Self::entry_name_key).collect();
        let paths: Vec<PathBuf> = entries.iter().map(|e| e.path()).collect();
        let sizes: Vec<u64>    = metas.iter()
            .enumerate()
            .map(|(idx, m)| {
                let default_size = m.as_ref().map(|m| m.len()).unwrap_or(0);
                if !matches!(mode, SortMode::SizeAsc | SortMode::SizeDesc) {
                    return default_size;
                }

                if is_dirs[idx] {
                    folder_size_cache
                        .and_then(|cache| cache.get(&paths[idx]).copied())
                        .unwrap_or(0)
                } else {
                    default_size
                }
            })
            .collect();
        let times: Vec<u64>    = metas.iter().map(|m| {
            m.as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0)
        }).collect();
        let exts: Vec<String>  = entries.iter().map(Self::entry_extension_key).collect();

        let mut indices: Vec<usize> = (0..entries.len()).collect();
        indices.sort_by(|&a, &b| {
            // Directories always sort before files.
            let type_ord = is_dirs[b].cmp(&is_dirs[a]);
            if type_ord != std::cmp::Ordering::Equal {
                return type_ord;
            }
            match mode {
                SortMode::NameAsc        => names[a].cmp(&names[b]),
                SortMode::NameDesc       => names[b].cmp(&names[a]),
                SortMode::ExtensionAsc   => exts[a].cmp(&exts[b]).then_with(|| names[a].cmp(&names[b])),
                SortMode::SizeAsc        => sizes[a].cmp(&sizes[b]).then_with(|| names[a].cmp(&names[b])),
                SortMode::SizeDesc       => sizes[b].cmp(&sizes[a]).then_with(|| names[a].cmp(&names[b])),
                SortMode::ModifiedNewest => times[b].cmp(&times[a]).then_with(|| names[a].cmp(&names[b])),
                SortMode::ModifiedOldest => times[a].cmp(&times[b]).then_with(|| names[a].cmp(&names[b])),
            }
        });

        // Rearrange entries in-place to match the sorted index permutation.
        let mut tmp: Vec<Option<fs::DirEntry>> = entries.drain(..).map(Some).collect();
        *entries = indices.into_iter().map(|i| tmp[i].take().unwrap()).collect();
    }

    pub(crate) fn apply_sort_to_current_entries(&mut self) {
        if !self.tree.expansion_levels.is_empty() {
            let selected_path = self.left.entries.get(self.left.selected_index).map(|e| e.path());
            let _ = self.refresh_entries();
            if let Some(path) = selected_path
                && let Some(idx) = self.left.entries.iter().position(|e| e.path() == path) {
                    self.left.selected_index = idx;
                    self.left.table_state.select(Some(idx));
                }
            return;
        }
        let selected_path = self.left.entries.get(self.left.selected_index).map(|e| e.path());
        let marked_paths: HashSet<PathBuf> = self.left
            .marked_indices
            .iter()
            .filter_map(|idx| self.left.entries.get(*idx).map(|e| e.path()))
            .collect();

        let folder_size_cache = if self.size.folder_size_enabled {
            Some(&self.size.folder_size_cache)
        } else {
            None
        };
        Self::sort_entries_by_mode(&mut self.left.entries, self.left.sort_mode, folder_size_cache);

        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
            filename_color_mode: self.filename_color_mode,
        };
        let uid_cache = App::build_uid_cache(&self.left.entries);
        let gid_cache = App::build_gid_cache(&self.left.entries);
            self.left.entry_render_cache = self.left.entries.iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        self.apply_cached_folder_size_columns();
        self.refresh_meta_identity_widths();

        self.left.marked_indices = self.left
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| marked_paths.contains(&entry.path()))
            .map(|(idx, _)| idx)
            .collect();

        if self.left.entries.is_empty() {
            self.left.selected_index = 0;
            self.left.table_state.select(None);
            return;
        }

        self.left.selected_index = selected_path
            .and_then(|p| self.left.entries.iter().position(|e| e.path() == p))
            .unwrap_or_else(|| self.left.selected_index.min(self.left.entries.len() - 1));
        self.left.table_state.select(Some(self.left.selected_index));
    }

    pub(crate) fn begin_sort_menu(&mut self) {
        self.panel_tab = 4;
        self.sort_menu_selected = Self::sort_mode_index(self.left.sort_mode);
        self.mode = AppMode::SortMenu;
    }

    pub(crate) fn commit_sort_menu_choice(&mut self) {
        let options = Self::sort_mode_options();
        if let Some(mode) = options.get(self.sort_menu_selected).copied() {
            self.left.sort_mode = mode;
            self.apply_sort_to_current_entries();
            self.set_status(format!("sort: {}", mode.label()));
        }
        self.mode = AppMode::Browsing;
    }
}
