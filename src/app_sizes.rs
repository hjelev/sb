use std::{
    collections::HashSet,
    fs, io,
    path::PathBuf,
    process::Command,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::UNIX_EPOCH,
};
use crate::util::background::{drain_channel, spawn_worker};

use rayon::prelude::*;

use crate::{
    App, CurrentDirTotalSizeMsg, FolderSizeMsg, RecursiveMtimeMsg, SelectedTotalSizeMsg,
};

impl App {
    fn active_size_context_dir(&self) -> PathBuf {
        if self.is_dual_panel_mode()
            && self.active_panel == crate::DualPanelSide::Right
            && !self.right.dir.as_os_str().is_empty()
        {
            self.right.dir.clone()
        } else {
            self.current_dir.clone()
        }
    }

    /// Signal a previous scan's worker to stop and install a fresh token.
    ///
    /// Flips the old `Arc<AtomicBool>` (if any) to `true` so the still-running
    /// recursive walk bails out early, then returns a new token cloned into the
    /// new worker. The cooperative checks live in the recursive walk functions.
    fn renew_cancel_token(slot: &mut Option<Arc<AtomicBool>>) -> Arc<AtomicBool> {
        if let Some(old) = slot.take() {
            old.store(true, Ordering::Relaxed);
        }
        let token = Arc::new(AtomicBool::new(false));
        *slot = Some(token.clone());
        token
    }

    /// Signal a previous scan's worker to stop without starting a new one.
    fn abort_cancel_token(slot: &mut Option<Arc<AtomicBool>>) {
        if let Some(old) = slot.take() {
            old.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn start_recursive_mtime_scan(&mut self) {
        self.recursive_mtime_scan_id = self.recursive_mtime_scan_id.wrapping_add(1);
        let scan_id = self.recursive_mtime_scan_id;
        let cancel = Self::renew_cancel_token(&mut self.recursive_mtime_cancel);

        let mut unique_dirs: HashSet<PathBuf> = self
            .entries
            .iter()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        if self.is_dual_panel_mode() {
            for path in self
                .right.entries
                .iter()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
            {
                unique_dirs.insert(path);
            }
        }
        let dir_paths: Vec<PathBuf> = unique_dirs.into_iter().collect();

        if dir_paths.is_empty() {
            self.recursive_mtime_rx = None;
            return;
        }

        self.recursive_mtime_rx = Some(spawn_worker(move |tx| {
            let updated: Vec<(PathBuf, u64)> = dir_paths
                .par_iter()
                .map(|dir| {
                    (
                        dir.clone(),
                        App::compute_latest_modified_unix_recursive(dir, Some(&cancel)).unwrap_or(0),
                    )
                })
                .collect();

            if cancel.load(Ordering::Relaxed) {
                return;
            }
            for (dir, latest_unix) in updated {
                let _ = tx.send(RecursiveMtimeMsg::EntryMtime(scan_id, dir, latest_unix));
            }
            let _ = tx.send(RecursiveMtimeMsg::Finished(scan_id));
        }));
    }

    pub(crate) fn pump_recursive_mtime_progress(&mut self) {
        for msg in drain_channel(&mut self.recursive_mtime_rx) {
            match msg {
                RecursiveMtimeMsg::EntryMtime(scan_id, dir_path, unix_secs) => {
                    if scan_id != self.recursive_mtime_scan_id {
                        continue;
                    }
                    if let Some(idx) = self.entries.iter().position(|e| e.path() == dir_path) {
                        self.entry_render_cache[idx].modified_unix = Some(unix_secs);
                        self.entry_render_cache[idx].date_col = format!(
                            "{:>width$}",
                            crate::util::format::format_mtime(
                                UNIX_EPOCH + std::time::Duration::from_secs(unix_secs)
                            ),
                            width = 16
                        );
                    }
                }
                RecursiveMtimeMsg::Finished(scan_id) => {
                    if scan_id == self.recursive_mtime_scan_id {
                        self.recursive_mtime_rx = None;
                    }
                }
            }
        }
    }

    pub(crate) fn compute_latest_modified_unix_recursive(
        path: &PathBuf,
        cancel: Option<&AtomicBool>,
    ) -> io::Result<u64> {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Ok(0);
        }
        let meta = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };

        let mut latest = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if !meta.file_type().is_dir() || meta.file_type().is_symlink() {
            return Ok(latest);
        }

        let children = match fs::read_dir(path) {
            Ok(rd) => rd,
            Err(_) => return Ok(latest),
        };

        for child in children.flatten() {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Ok(latest);
            }
            let child_path = child.path();
            let child_latest =
                Self::compute_latest_modified_unix_recursive(&child_path, cancel).unwrap_or(0);
            latest = latest.max(child_latest);
        }

        Ok(latest)
    }

    pub(crate) fn clear_selected_total_size_state_for(&mut self, side: crate::DualPanelSide) {
        match side {
            crate::DualPanelSide::Left => {
                self.selected_total_size_scan_id = self.selected_total_size_scan_id.wrapping_add(1);
                self.selected_total_size_rx = None;
                self.selected_total_size_pending = false;
                self.selected_total_size_bytes = None;
                self.selected_total_size_items = 0;
            }
            crate::DualPanelSide::Right => {
                self.right.selected_total_size_scan_id = self.right.selected_total_size_scan_id.wrapping_add(1);
                self.right.selected_total_size_rx = None;
                self.right.selected_total_size_pending = false;
                self.right.selected_total_size_bytes = None;
                self.right.selected_total_size_items = 0;
            }
        }
    }

    pub(crate) fn clear_selected_total_size_state(&mut self) {
        let side = if self.is_dual_panel_mode() && self.active_panel == crate::DualPanelSide::Right {
            crate::DualPanelSide::Right
        } else {
            crate::DualPanelSide::Left
        };
        self.clear_selected_total_size_state_for(side);
    }

    pub(crate) fn start_selected_total_size_scan(&mut self) {
        let side = if self.is_dual_panel_mode() && self.active_panel == crate::DualPanelSide::Right {
            crate::DualPanelSide::Right
        } else {
            crate::DualPanelSide::Left
        };

        let targets: Vec<PathBuf> = match side {
            crate::DualPanelSide::Left => {
                if !self.folder_size_enabled || self.marked_indices.len() < 2 {
                    self.clear_selected_total_size_state_for(side);
                    return;
                }

                self.entries
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| self.marked_indices.contains(i))
                    .map(|(_, e)| e.path())
                    .collect()
            }
            crate::DualPanelSide::Right => {
                if !self.folder_size_enabled || self.right.marked_indices.len() < 2 {
                    self.clear_selected_total_size_state_for(side);
                    return;
                }

                self.right.entries
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| self.right.marked_indices.contains(i))
                    .map(|(_, e)| e.path())
                    .collect()
            }
        };

        if targets.len() < 2 {
            self.clear_selected_total_size_state_for(side);
            return;
        }

        let (tx, rx) = mpsc::channel();
        let scan_id = match side {
            crate::DualPanelSide::Left => {
                self.selected_total_size_scan_id = self.selected_total_size_scan_id.wrapping_add(1);
                self.selected_total_size_items = targets.len();
                self.selected_total_size_pending = true;
                self.selected_total_size_bytes = None;
                self.selected_total_size_rx = Some(rx);
                self.selected_total_size_scan_id
            }
            crate::DualPanelSide::Right => {
                self.right.selected_total_size_scan_id = self.right.selected_total_size_scan_id.wrapping_add(1);
                self.right.selected_total_size_items = targets.len();
                self.right.selected_total_size_pending = true;
                self.right.selected_total_size_bytes = None;
                self.right.selected_total_size_rx = Some(rx);
                self.right.selected_total_size_scan_id
            }
        };

        thread::spawn(move || {
            let total = targets
                .par_iter()
                .map(|p| App::compute_total_display_bytes(p, None).unwrap_or(0))
                .reduce(|| 0u64, |acc, v| acc.saturating_add(v));
            let _ = tx.send(SelectedTotalSizeMsg::Finished(scan_id, total));
        });
    }

    pub(crate) fn pump_selected_total_size_progress(&mut self) {
        for msg in drain_channel(&mut self.selected_total_size_rx) {
            let SelectedTotalSizeMsg::Finished(scan_id, bytes) = msg;
            if scan_id == self.selected_total_size_scan_id {
                self.selected_total_size_bytes = Some(bytes);
                self.selected_total_size_pending = false;
                self.selected_total_size_rx = None;
            }
        }
        if !self.folder_size_enabled {
            self.selected_total_size_rx = None;
        }

        for msg in drain_channel(&mut self.right.selected_total_size_rx) {
            let SelectedTotalSizeMsg::Finished(scan_id, bytes) = msg;
            if scan_id == self.right.selected_total_size_scan_id {
                self.right.selected_total_size_bytes = Some(bytes);
                self.right.selected_total_size_pending = false;
                self.right.selected_total_size_rx = None;
            }
        }
        if !self.folder_size_enabled {
            self.right.selected_total_size_rx = None;
        }
    }

    pub(crate) fn selected_total_size_status(&self) -> Option<String> {
        let side = if self.is_dual_panel_mode() && self.active_panel == crate::DualPanelSide::Right {
            crate::DualPanelSide::Right
        } else {
            crate::DualPanelSide::Left
        };
        self.selected_total_size_status_for(side)
    }

    pub(crate) fn selected_total_size_status_for(&self, side: crate::DualPanelSide) -> Option<String> {
        let (selected_count, pending, bytes, items) = match side {
            crate::DualPanelSide::Left => (
                self.marked_indices.len(),
                self.selected_total_size_pending,
                self.selected_total_size_bytes,
                self.selected_total_size_items,
            ),
            crate::DualPanelSide::Right => (
                self.right.marked_indices.len(),
                self.right.selected_total_size_pending,
                self.right.selected_total_size_bytes,
                self.right.selected_total_size_items,
            ),
        };

        if selected_count == 0 {
            return None;
        }

        let noun = if selected_count == 1 { "item" } else { "items" };
        if !self.folder_size_enabled || selected_count < 2 {
            return Some(format!("selected: {} {}", selected_count, noun));
        }

        if pending {
            return Some(format!(
                "selected: {} {} | total size: scanning...",
                items.max(selected_count),
                noun
            ));
        }

        Some(match bytes {
            Some(bytes) => format!(
                "selected: {} {} | total size: {}",
                items.max(selected_count),
                noun,
                Self::format_size(bytes)
            ),
            None => format!("selected: {} {}", selected_count, noun),
        })
    }

    pub(crate) fn start_folder_size_scan(&mut self) {
        if !self.folder_size_enabled {
            return;
        }

        self.folder_size_scan_id = self.folder_size_scan_id.wrapping_add(1);
        let scan_id = self.folder_size_scan_id;
        let cancel = Self::renew_cancel_token(&mut self.folder_size_cancel);

        let mut unique_dirs: HashSet<PathBuf> = self
            .entries
            .iter()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        if self.is_dual_panel_mode() {
            for path in self
                .right.entries
                .iter()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
            {
                unique_dirs.insert(path);
            }
        }
        let dir_paths: Vec<PathBuf> = unique_dirs.into_iter().collect();

        if dir_paths.is_empty() {
            self.folder_size_rx = None;
            return;
        }

        self.folder_size_rx = Some(spawn_worker(move |tx| {
            let sized: Vec<(PathBuf, u64)> = dir_paths
                .par_iter()
                .map(|dir| (dir.clone(), App::compute_total_display_bytes(dir, Some(&cancel)).unwrap_or(0)))
                .collect();
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            for (dir, size) in sized {
                let _ = tx.send(FolderSizeMsg::EntrySize(scan_id, dir, size));
            }
            let _ = tx.send(FolderSizeMsg::Finished(scan_id));
        }));
    }

    pub(crate) fn clear_current_dir_total_size_state(&mut self) {
        self.current_dir_total_size_scan_id = self.current_dir_total_size_scan_id.wrapping_add(1);
        Self::abort_cancel_token(&mut self.current_dir_total_size_cancel);
        self.current_dir_total_size_rx = None;
        self.current_dir_total_size_pending = false;
        self.current_dir_total_size_bytes = None;
    }

    pub(crate) fn filesystem_space_info(path: &PathBuf) -> Option<(u64, u64)> {
        let output = Command::new("df").args(["-kP"]).arg(path).output().ok()?;
        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().rev().find(|line| !line.trim().is_empty())?;
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            return None;
        }

        let total_kb = u64::from_str(cols[1]).ok()?;
        let available_kb = u64::from_str(cols[3]).ok()?;
        Some((total_kb.saturating_mul(1024), available_kb.saturating_mul(1024)))
    }

    pub(crate) fn refresh_current_dir_free_space(&mut self) {
        let context_dir = self.active_size_context_dir();
        if let Some((total, free)) = Self::filesystem_space_info(&context_dir) {
            self.current_dir_total_space_bytes = Some(total);
            self.current_dir_free_bytes = Some(free);
        } else {
            self.current_dir_total_space_bytes = None;
            self.current_dir_free_bytes = None;
        }
    }

    pub(crate) fn start_current_dir_total_size_scan(&mut self) {
        if !self.folder_size_enabled {
            return;
        }

        self.current_dir_total_size_scan_id = self.current_dir_total_size_scan_id.wrapping_add(1);
        let scan_id = self.current_dir_total_size_scan_id;
        let cancel = Self::renew_cancel_token(&mut self.current_dir_total_size_cancel);
        let current_dir = self.active_size_context_dir();
        self.current_dir_total_size_pending = true;
        self.current_dir_total_size_bytes = None;

        self.current_dir_total_size_rx = Some(spawn_worker(move |tx| {
            let total = App::compute_total_display_bytes(&current_dir, Some(&cancel)).unwrap_or(0);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let _ = tx.send(CurrentDirTotalSizeMsg::Finished(scan_id, total));
        }));
    }

    pub(crate) fn pump_current_dir_total_size_progress(&mut self) {
        for msg in drain_channel(&mut self.current_dir_total_size_rx) {
            let CurrentDirTotalSizeMsg::Finished(scan_id, bytes) = msg;
            if scan_id == self.current_dir_total_size_scan_id {
                self.current_dir_total_size_bytes = Some(bytes);
                self.current_dir_total_size_pending = false;
                self.current_dir_total_size_rx = None;
            }
        }
        if !self.folder_size_enabled {
            self.current_dir_total_size_rx = None;
        }
    }

    pub(crate) fn current_dir_total_size_header_info(&self) -> Option<crate::DiskHeaderInfo> {
        // Shown when folder-size mode is on, or when the clock is disabled (in
        // which case the disk pill replaces it, without the folder-size prefix).
        if !self.folder_size_enabled && !self.disable_clock {
            return None;
        }
        let (folder_label, disk_label) = if self.nerd_font_active {
            ("\u{f10b7}", "\u{f02ca}")
        } else {
            ("folder:", "disk:")
        };
        let total_raw = self.current_dir_total_space_bytes;
        let free_raw = self.current_dir_free_bytes;
        let used_raw = match (total_raw, free_raw) {
            (Some(total), Some(free)) => Some(total.saturating_sub(free)),
            _ => None,
        };
        let used_fraction = match (total_raw, used_raw) {
            (Some(total), Some(used)) if total > 0 => Some((used as f64 / total as f64).clamp(0.0, 1.0)),
            _ => None,
        };

        let total_space = total_raw.map(Self::format_size);
        let disk_segment = match (used_raw, total_space) {
            (Some(used), Some(total)) => format!("{} {} / {}", disk_label, Self::format_size(used), total),
            (Some(used), None) => format!("{} {} / ?", disk_label, Self::format_size(used)),
            (None, Some(total)) => format!("{} ? / {}", disk_label, total),
            (None, None) => format!("{} ? / ?", disk_label),
        };

        // Recursive folder-size prefix: only shown when folder-size mode is on.
        // When the pill is shown solely because the clock is disabled, there is
        // no prefix. Trailing space leaves one uncolored gap before the bar.
        let folder_segment = if !self.folder_size_enabled {
            String::new()
        } else if self.current_dir_total_size_pending {
            format!("{} scanning... ", folder_label)
        } else {
            match self.current_dir_total_size_bytes {
                Some(bytes) => format!("{} {} ", folder_label, Self::format_size(bytes)),
                None => format!("{} ? ", folder_label),
            }
        };

        Some(crate::DiskHeaderInfo {
            folder_segment,
            disk_segment,
            used_fraction,
        })
    }

    pub(crate) fn reset_folder_size_columns(&mut self) {
        let size_width = 6usize;
        for (idx, entry) in self.entries.iter().enumerate() {
            if entry.path().is_dir() {
                self.entry_render_cache[idx].size_col = format!("{:>width$}", "-", width = size_width);
                self.entry_render_cache[idx].size_bytes = None;
            }
        }
        for (idx, entry) in self.right.entries.iter().enumerate() {
            if entry.path().is_dir() {
                self.right.entry_render_cache[idx].size_col = format!("{:>width$}", "-", width = size_width);
                self.right.entry_render_cache[idx].size_bytes = None;
            }
        }
    }

    pub(crate) fn apply_cached_folder_size_columns(&mut self) {
        for (idx, entry) in self.entries.iter().enumerate() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(size) = self.folder_size_cache.get(&path).copied() {
                self.entry_render_cache[idx].size_col =
                    format!("{:>width$}", Self::format_size(size), width = 6);
                self.entry_render_cache[idx].size_bytes = Some(size);
            }
        }
        for (idx, entry) in self.right.entries.iter().enumerate() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(size) = self.folder_size_cache.get(&path).copied() {
                self.right.entry_render_cache[idx].size_col =
                    format!("{:>width$}", Self::format_size(size), width = 6);
                self.right.entry_render_cache[idx].size_bytes = Some(size);
            }
        }
    }

    pub(crate) fn set_folder_size_enabled(&mut self, enabled: bool) {
        if enabled == self.folder_size_enabled {
            return;
        }

        self.folder_size_enabled = enabled;
        self.folder_size_scan_id = self.folder_size_scan_id.wrapping_add(1);
        Self::abort_cancel_token(&mut self.folder_size_cancel);
        self.folder_size_rx = None;
        self.reset_folder_size_columns();

        // Persist the choice so it is restored on next launch.
        let mut cfg = crate::util::config::SbPersistConfig::load();
        cfg.folder_size_enabled = enabled;
        let _ = cfg.save();

        if enabled {
            self.apply_cached_folder_size_columns();
            self.set_status("folder size calculation: on");
            self.start_folder_size_scan();
            self.start_current_dir_total_size_scan();
            self.start_selected_total_size_scan();
        } else {
            self.set_status("folder size calculation: off");
            self.clear_current_dir_total_size_state();
            self.clear_selected_total_size_state();
        }
    }

    pub(crate) fn pump_folder_size_progress(&mut self) {
        let mut any_size_changed = false;
        for msg in drain_channel(&mut self.folder_size_rx) {
            match msg {
                FolderSizeMsg::EntrySize(scan_id, dir_path, size) => {
                    if !self.folder_size_enabled || scan_id != self.folder_size_scan_id {
                        continue;
                    }
                    let previous = self.folder_size_cache.insert(dir_path.clone(), size);
                    if previous != Some(size) {
                        any_size_changed = true;
                    }
                    if let Some(idx) = self.entries.iter().position(|e| e.path() == dir_path) {
                        self.entry_render_cache[idx].size_col =
                            format!("{:>width$}", Self::format_size(size), width = 6);
                        self.entry_render_cache[idx].size_bytes = Some(size);
                    }
                    if let Some(idx) = self.right.entries.iter().position(|e| e.path() == dir_path) {
                        self.right.entry_render_cache[idx].size_col =
                            format!("{:>width$}", Self::format_size(size), width = 6);
                        self.right.entry_render_cache[idx].size_bytes = Some(size);
                    }
                }
                FolderSizeMsg::Finished(scan_id) => {
                    if scan_id == self.folder_size_scan_id {
                        self.folder_size_rx = None;
                    }
                }
            }
        }
        if any_size_changed && matches!(self.sort_mode, crate::SortMode::SizeAsc | crate::SortMode::SizeDesc) {
            self.apply_sort_to_current_entries();
        }
        if !self.folder_size_enabled {
            self.folder_size_rx = None;
        }
    }

    pub(crate) fn compute_total_bytes(src: &PathBuf) -> io::Result<u64> {
        Self::compute_total_bytes_inner(src, true)
    }

    pub(crate) fn compute_total_display_bytes(
        src: &PathBuf,
        cancel: Option<&AtomicBool>,
    ) -> io::Result<u64> {
        Self::compute_total_display_bytes_inner(src, false, cancel)
    }

    pub(crate) fn compute_total_bytes_inner(src: &PathBuf, follow_symlink_dir: bool) -> io::Result<u64> {
        // Best-effort size walk: skip unreadable nodes instead of failing the whole tree.
        let metadata = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };

        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            if follow_symlink_dir
                && let Ok(target_meta) = fs::metadata(src)
                    && target_meta.is_dir() {
                        return Self::compute_dir_total_bytes(src);
                    }
            return Ok(metadata.len());
        }

        if file_type.is_dir() {
            return Self::compute_dir_total_bytes(src);
        }

        Ok(metadata.len())
    }

    pub(crate) fn compute_total_display_bytes_inner(
        src: &PathBuf,
        follow_symlink_dir: bool,
        cancel: Option<&AtomicBool>,
    ) -> io::Result<u64> {
        // Best-effort size walk for display: uses disk-usage bytes on Unix to avoid
        // huge apparent sizes from virtual files (for example /proc/kcore).
        let metadata = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };

        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            if follow_symlink_dir
                && let Ok(target_meta) = fs::metadata(src)
                    && target_meta.is_dir() {
                        return Self::compute_dir_total_display_bytes(src, cancel);
                    }
            return Ok(Self::display_leaf_size(&metadata));
        }

        if file_type.is_dir() {
            return Self::compute_dir_total_display_bytes(src, cancel);
        }

        Ok(Self::display_leaf_size(&metadata))
    }

    pub(crate) fn compute_dir_total_bytes(dir: &PathBuf) -> io::Result<u64> {
        const SIZE_WALK_PAR_THRESHOLD: usize = 32;
        let children = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return Ok(0),
        };

        let child_paths: Vec<PathBuf> = children
            .filter_map(|child| child.ok().map(|entry| entry.path()))
            .collect();

        let total = if child_paths.len() >= SIZE_WALK_PAR_THRESHOLD {
            child_paths
                .par_iter()
                .map(|child_path| Self::compute_total_bytes_inner(child_path, false).unwrap_or(0))
                .reduce(|| 0u64, |acc, v| acc.saturating_add(v))
        } else {
            child_paths
                .iter()
                .map(|child_path| Self::compute_total_bytes_inner(child_path, false).unwrap_or(0))
                .fold(0u64, |acc, v| acc.saturating_add(v))
        };

        Ok(total)
    }

    pub(crate) fn compute_dir_total_display_bytes(
        dir: &PathBuf,
        cancel: Option<&AtomicBool>,
    ) -> io::Result<u64> {
        const SIZE_WALK_PAR_THRESHOLD: usize = 32;
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Ok(0);
        }
        let children = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return Ok(0),
        };

        let child_paths: Vec<PathBuf> = children
            .filter_map(|child| child.ok().map(|entry| entry.path()))
            .collect();

        let total = if child_paths.len() >= SIZE_WALK_PAR_THRESHOLD {
            child_paths
                .par_iter()
                .map(|child_path| Self::compute_total_display_bytes_inner(child_path, false, cancel).unwrap_or(0))
                .reduce(|| 0u64, |acc, v| acc.saturating_add(v))
        } else {
            child_paths
                .iter()
                .map(|child_path| Self::compute_total_display_bytes_inner(child_path, false, cancel).unwrap_or(0))
                .fold(0u64, |acc, v| acc.saturating_add(v))
        };

        Ok(total)
    }

    pub(crate) fn display_leaf_size(metadata: &fs::Metadata) -> u64 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            metadata.blocks().saturating_mul(512)
        }
        #[cfg(not(unix))]
        {
            metadata.len()
        }
    }
}
