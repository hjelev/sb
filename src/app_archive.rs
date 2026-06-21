use std::{
    fs, io,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use crate::util::background::{drain_channel, spawn_worker};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::{App, AppMode, ArchiveKind, ArchiveMount, ArchiveProgressMsg};
use crate::util::command::CommandBuilder;
use crate::util::cleanup::safe_cleanup_path;

impl App {
    pub(crate) fn create_archive_mount_path(&self) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("sbrs_zip_{}_{}", std::process::id(), stamp))
    }

    pub(crate) fn try_mount_archive(&mut self, archive_path: PathBuf) -> bool {
        self.try_mount_archive_with(archive_path, "fuse-zip")
    }

    pub(crate) fn try_mount_archive_with(&mut self, archive_path: PathBuf, tool: &str) -> bool {
        if !self.integration_active(tool) {
            self.set_status(format!("{} not installed", tool));
            return false;
        }

        if let Some(existing_idx) = self
            .archive_mounts
            .iter()
            .position(|m| m.archive_path == archive_path && m.mount_path.is_dir())
        {
            let archive_name = crate::util::classify::display_name(&archive_path);
            let mount_path = self.archive_mounts[existing_idx].mount_path.clone();
            self.archive_mounts[existing_idx].return_dir = self.active_panel_dir();
            self.archive_mounts[existing_idx].archive_name = archive_name;
            self.try_enter_dir_on_active_panel(mount_path);
            return true;
        }

        let mount_path = self.create_archive_mount_path();
        if fs::create_dir_all(&mount_path).is_err() {
            self.set_status("failed to create archive mount directory");
            return false;
        }

        match Command::new(tool).arg(&archive_path).arg(&mount_path).status() {
            Ok(status) if status.success() => {
                let archive_name = crate::util::classify::display_name(&archive_path);
                let return_dir = self.active_panel_dir();
                self.archive_mounts.push(ArchiveMount {
                    archive_path,
                    mount_path: mount_path.clone(),
                    return_dir,
                    archive_name,
                });
                self.try_enter_dir_on_active_panel(mount_path);
                true
            }
            _ => {
                let _ = safe_cleanup_path(&mount_path);
                self.set_status(format!("failed to mount archive with {}", tool));
                false
            }
        }
    }

    pub(crate) fn preview_archive_contents(&mut self, archive_path: &PathBuf) -> bool {
        let archive_name = crate::util::classify::display_name(archive_path);

        let mut cmd = match Self::archive_kind(archive_path) {
            Some(ArchiveKind::Zip)
                if self.integration_enabled("zip") && Self::integration_probe("unzip").0 =>
            {
                let mut c = Command::new("unzip");
                c.arg("-l").arg(archive_path);
                c
            }
            Some(ArchiveKind::Tar) if self.integration_active("tar") => {
                let mut c = Command::new("tar");
                c.arg("-tvf").arg(archive_path);
                c
            }
            Some(ArchiveKind::SevenZip)
                if self.integration_enabled("7z") && Self::seven_zip_tool().is_some() =>
            {
                let tool = Self::seven_zip_tool().unwrap_or_else(|| "7z".to_string());
                let mut c = Command::new(tool);
                c.arg("l").arg(archive_path);
                c
            }
            Some(ArchiveKind::Rar)
                if self.integration_enabled("rar") && Self::rar_tool().is_some() =>
            {
                let tool = Self::rar_tool().unwrap_or_else(|| "unrar".to_string());
                let mut c = Command::new(tool);
                c.arg("l").arg(archive_path);
                c
            }
            _ => {
                self.set_status(format!(
                    "no archive preview tool available for {}",
                    archive_name
                ));
                return false;
            }
        };

        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);

        let mut shown = false;
        if let Ok(mut child) = cmd.stdout(Stdio::piped()).spawn() {
            if let Some(stdout) = child.stdout.take() {
                shown = Command::new("less")
                    .arg("-R")
                    .stdin(stdout)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            }
            let _ = child.wait();
        }

        let _ = execute!(io::stdout(), EnterAlternateScreen);
        let _ = enable_raw_mode();

        if shown {
            self.set_status(format!("previewed archive listing: {}", archive_name));
        } else {
            self.set_status(format!("failed to preview archive: {}", archive_name));
        }

        shown
    }

    pub(crate) fn unmount_archive_path(path: &PathBuf) {
        // Best-effort unmount — ignore failure since we'll remove the dir anyway.
        let _ = CommandBuilder::unmount_archive(path);
    }

    pub(crate) fn try_leave_archive(&mut self) -> bool {
        if self.is_dual_panel_mode() && self.active_panel == crate::DualPanelSide::Right {
            let Some(mount_idx) = self
                .archive_mounts
                .iter()
                .rposition(|mount| mount.mount_path == self.right.dir)
            else {
                return false;
            };

            let return_dir = self.archive_mounts[mount_idx].return_dir.clone();
            let archive_name = self.archive_mounts[mount_idx].archive_name.clone();
            self.right.dir = return_dir;
            if self.refresh_right_panel_entries().is_ok() {
                self.select_right_entry_named(&archive_name);
            }
            return true;
        }

        let Some(mount_idx) = self
            .archive_mounts
            .iter()
            .rposition(|mount| mount.mount_path == self.current_dir)
        else {
            return false;
        };

        self.remember_current_selection();
        let return_dir = self.archive_mounts[mount_idx].return_dir.clone();
        let archive_name = self.archive_mounts[mount_idx].archive_name.clone();
        self.current_dir = return_dir;
        if self.refresh_entries_or_status() {
            self.select_entry_named(&archive_name);
        }
        true
    }

    pub(crate) fn cleanup_archive_mounts(&mut self) {
        // If current_dir is inside an archive mount, switch back to that mount's
        // return directory before unmounting so shell integration doesn't keep
        // a now-removed temp path.
        if let Some(mount) = self
            .archive_mounts
            .iter()
            .rev()
            .find(|m| self.current_dir == m.mount_path || self.current_dir.starts_with(&m.mount_path))
        {
            self.current_dir = mount.return_dir.clone();
        }

        while let Some(mount) = self.archive_mounts.pop() {
            let _ = mount.archive_path;
            Self::unmount_archive_path(&mount.mount_path);
            let _ = safe_cleanup_path(&mount.mount_path);
        }
    }

    pub(crate) fn unmount_archive_mount_by_path(&mut self, mount_path: &PathBuf) -> bool {
        let Some(idx) = self
            .archive_mounts
            .iter()
            .rposition(|m| &m.mount_path == mount_path)
        else {
            return false;
        };

        let mount = self.archive_mounts.remove(idx);
        let was_inside = self.current_dir == mount.mount_path || self.current_dir.starts_with(&mount.mount_path);
        if was_inside {
            self.current_dir = mount.return_dir.clone();
            if self.refresh_entries_or_status() {
                self.select_entry_named(&mount.archive_name);
            }
        }
        Self::unmount_archive_path(&mount.mount_path);
        let _ = safe_cleanup_path(&mount.mount_path);
        true
    }

    pub(crate) fn archive_targets(&self) -> Vec<PathBuf> {
        if self.is_dual_panel_mode() && self.active_panel == crate::DualPanelSide::Right {
            if !self.right.marked_indices.is_empty() {
                self.right.entries
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| self.right.marked_indices.contains(i))
                    .map(|(_, e)| e.path())
                    .collect()
            } else {
                self.right.entries
                    .get(self.right.selected_index)
                    .map(|e| e.path())
                    .into_iter()
                    .collect()
            }
        } else if !self.marked_indices.is_empty() {
            self.entries
                .iter()
                .enumerate()
                .filter(|(i, _)| self.marked_indices.contains(i))
                .map(|(_, e)| e.path())
                .collect()
        } else {
            self.entries
                .get(self.selected_index)
                .map(|e| e.path())
                .into_iter()
                .collect()
        }
    }

    pub(crate) fn run_zip_action(&mut self) {
        if self.archive.rx.is_some() {
            self.set_status("archive creation already in progress");
            return;
        }

        let targets = self.archive_targets();
        if targets.is_empty() {
            self.set_status("no selected item");
            return;
        }

        let all_archives = targets.iter().all(Self::is_supported_archive);

        if all_archives {
            if targets.iter().any(|p| !self.can_extract_archive(p)) {
                self.set_status("missing extractor for one or more selected archives");
                return;
            }

            self.archive.extract_targets = targets;
            self.mode = AppMode::ConfirmExtract;
            self.set_status("confirm extraction: press y to continue");
            return;
        }

        if !self.integration_enabled("zip") || !Self::integration_probe("zip").0 {
            self.status_tool_not_found("zip");
            return;
        }

        let base_name = if targets.len() == 1 {
            targets[0]
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "archive".to_string())
        } else {
            "archive".to_string()
        };
        let mut archive_name = format!("{}.zip", base_name);
        let mut n = 2usize;
        while self.current_dir.join(&archive_name).exists() {
            archive_name = format!("{}-{}.zip", base_name, n);
            n += 1;
        }

        self.archive.create_targets = targets;
        self.begin_input_edit(AppMode::ArchiveCreate, archive_name);
        self.set_status("confirm archive name and press Enter");
    }

    pub(crate) fn create_archive_from_input(&mut self) {
        if self.archive.rx.is_some() {
            self.set_status("archive creation already in progress");
            return;
        }

        let mut archive_name = self.input_buffer.trim().to_string();
        if archive_name.is_empty() {
            self.set_status("archive name cannot be empty");
            return;
        }
        if !archive_name.to_lowercase().ends_with(".zip") {
            archive_name.push_str(".zip");
        }

        let targets = self.archive.create_targets.clone();
        if targets.is_empty() {
            self.mode = AppMode::Browsing;
            self.clear_input_edit();
            self.set_status("nothing to archive");
            return;
        }

        if self.current_dir.join(&archive_name).exists() {
            self.set_status("archive already exists: choose another name");
            return;
        }

        let mut item_names: Vec<String> = Vec::new();
        for t in &targets {
            if let Some(name) = t.file_name() {
                item_names.push(name.to_string_lossy().into_owned());
            }
        }
        if item_names.is_empty() {
            self.mode = AppMode::Browsing;
            self.archive.create_targets.clear();
            self.clear_input_edit();
            self.set_status("nothing to archive");
            return;
        }

        self.mode = AppMode::Browsing;
        let targets = std::mem::take(&mut self.archive.create_targets);
        self.clear_input_edit();
        self.start_archive_job(archive_name, targets);
    }

    pub(crate) fn extract_archives_confirmed(&mut self) {
        let targets = std::mem::take(&mut self.archive.extract_targets);
        if targets.is_empty() {
            self.set_status("no archives selected");
            return;
        }

        let mut ok_count = 0usize;
        let mut fail_count = 0usize;
        for archive in &targets {
            let base = archive
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "extracted".to_string());

            let mut out_dir = self.current_dir.join(&base);
            let mut n = 2usize;
            while out_dir.exists() {
                out_dir = self.current_dir.join(format!("{}-{}", base, n));
                n += 1;
            }

            let _ = fs::create_dir_all(&out_dir);
            let ok = match Self::archive_kind(archive) {
                Some(ArchiveKind::Zip) => Command::new("unzip")
                    .args(["-q"])
                    .arg(archive)
                    .args(["-d"])
                    .arg(&out_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false),
                Some(ArchiveKind::Tar) => Command::new("tar")
                    .arg("-xf")
                    .arg(archive)
                    .arg("-C")
                    .arg(&out_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false),
                Some(ArchiveKind::SevenZip) => {
                    if let Some(tool) = Self::seven_zip_tool() {
                        Command::new(tool)
                            .arg("x")
                            .arg("-y")
                            .arg(format!("-o{}", out_dir.to_string_lossy()))
                            .arg(archive)
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
                Some(ArchiveKind::Rar) => {
                    if let Some(tool) = Self::rar_tool() {
                        Command::new(tool)
                            .arg("x")
                            .arg("-o+")
                            .arg(archive)
                            .arg(&out_dir)
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
                None => false,
            };
            if ok {
                ok_count += 1;
            } else {
                fail_count += 1;
            }
        }

        self.refresh_entries_or_status();
        self.sync_inactive_panel_if_same_dir();
        if fail_count == 0 {
            self.set_status(format!("extracted {} archive(s)", ok_count));
        } else {
            self.set_status(format!(
                "extract finished: {} ok, {} failed",
                ok_count, fail_count
            ));
        }
    }

    pub(crate) fn update_archive_status(&mut self) {
        if self.archive.name.is_empty() {
            return;
        }

        let total = self.archive.total_bytes;
        let done = self.archive.done_bytes;
        let scanning = total == 0 && done == 0;
        let display_total = total.max(done).max(1);
        let percent = if total == 0 {
            0.0
        } else {
            (done.min(display_total) as f64 * 100.0) / display_total as f64
        };

        let bar = crate::util::format::progress_bar(percent, 20);

        let elapsed = self
            .archive.started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
            .max(0.001);
        let speed = done as f64 / elapsed;
        let speed_str = if speed > 0.0 {
            format!("{}/s", Self::format_size(speed as u64))
        } else {
            "-".to_string()
        };

        let eta = if speed > 0.0 && display_total > done {
            let eta_secs = ((display_total - done) as f64 / speed).round() as u64;
            Self::format_eta(eta_secs)
        } else {
            "-".to_string()
        };
        let total_label = if scanning {
            "?".to_string()
        } else {
            Self::format_size(display_total)
        };
        let scan_suffix = if scanning { " scanning size..." } else { "" };

        self.set_status(format!(
            "archive [{}] {:>3.0}% {}/{} {} eta {} {}{}",
            bar,
            percent,
            Self::format_size(done),
            total_label,
            speed_str,
            eta,
            self.archive.name,
            scan_suffix
        ));
    }

    pub(crate) fn start_archive_job(&mut self, archive_name: String, targets: Vec<PathBuf>) {
        let mut item_names: Vec<String> = Vec::new();
        for t in &targets {
            if let Some(name) = t.file_name() {
                item_names.push(name.to_string_lossy().into_owned());
            }
        }
        if item_names.is_empty() {
            self.set_status("nothing to archive");
            return;
        }

        let cwd = self.current_dir.clone();
        let archive_path = cwd.join(&archive_name);
        self.archive.total_bytes = 0;
        self.archive.done_bytes = 0;
        self.archive.started_at = Some(Instant::now());
        self.archive.name = archive_name.clone();
        self.update_archive_status();

        self.archive.rx = Some(spawn_worker(move |tx| {
            let total_bytes = targets
                .iter()
                .filter_map(|p| Self::compute_total_bytes(p).ok())
                .fold(0u64, |acc, v| acc.saturating_add(v));
            let _ = tx.send(ArchiveProgressMsg::TotalBytes(total_bytes));

            let mut cmd = Command::new("zip");
            cmd.arg("-r")
                .arg(&archive_name)
                .args(&item_names)
                .current_dir(&cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());

            match cmd.spawn() {
                Ok(mut child) => loop {
                    let done = fs::metadata(&archive_path).map(|m| m.len()).unwrap_or(0);
                    let _ = tx.send(ArchiveProgressMsg::Progress(done));
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            if status.success() {
                                let _ = tx.send(ArchiveProgressMsg::Finished(Ok(archive_name.clone())));
                            } else {
                                let _ = tx.send(ArchiveProgressMsg::Finished(Err(
                                    "zip command failed".to_string(),
                                )));
                            }
                            break;
                        }
                        Ok(None) => {
                            thread::sleep(Duration::from_millis(120));
                        }
                        Err(e) => {
                            let _ = tx.send(ArchiveProgressMsg::Finished(Err(e.to_string())));
                            break;
                        }
                    }
                },
                Err(e) => {
                    let _ = tx.send(ArchiveProgressMsg::Finished(Err(e.to_string())));
                }
            }
        }));
    }

    pub(crate) fn pump_archive_progress(&mut self) {
        let mut finished: Option<Result<String, String>> = None;
        for msg in drain_channel(&mut self.archive.rx) {
            match msg {
                ArchiveProgressMsg::TotalBytes(total) => self.archive.total_bytes = total,
                ArchiveProgressMsg::Progress(done) => self.archive.done_bytes = done,
                ArchiveProgressMsg::Finished(result) => {
                    finished = Some(result);
                    self.archive.rx = None;
                }
            }
        }

        if let Some(result) = finished {
            self.archive.started_at = None;
            self.archive.total_bytes = 0;
            self.archive.done_bytes = 0;
            self.archive.name.clear();
            match result {
                Ok(name) => {
                    self.refresh_entries_or_status();
                    self.select_entry_named(&name);
                    self.sync_inactive_panel_if_same_dir();
                    self.set_status(format!("archive created: {}", name));
                }
                Err(e) => {
                    self.set_status(format!("archive create failed: {}", e));
                }
            }
        } else if self.archive.rx.is_some() {
            self.update_archive_status();
        }
    }
}
