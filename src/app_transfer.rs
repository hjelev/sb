use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::Sender,
    time::Instant,
};

use crate::util::background::{drain_channel, pump_once, spawn_worker};
use crate::util::tui::{self, ResumeMode};
use crate::{util, App, AppMode, CopyProgressMsg, DualPanelSide};

impl App {

    pub(crate) fn is_path_inside_remote_mount(&self, path: &PathBuf) -> bool {
        self.remote.ssh_mounts
            .iter()
            .any(|m| path == &m.mount_path || path.starts_with(&m.mount_path))
    }

    pub(crate) fn begin_transfer_from_sources(
        &mut self,
        sources: Vec<PathBuf>,
        target_dir: PathBuf,
        move_mode: bool,
    ) {
        if sources.is_empty() {
            self.set_status("no selected item");
            return;
        }
        if self.archive.rx.is_some() {
            self.set_status("archive creation in progress");
            return;
        }
        if self.copy.rx.is_some() {
            self.set_status("copy already in progress");
            return;
        }
        self.transfer.paste_queue = sources.iter().cloned().collect();
        self.transfer.paste_current_src = None;
        self.transfer.paste_move_mode = move_mode;
        self.transfer.paste_target_dir = Some(target_dir);
        self.transfer.paste_total_items = sources.len();
        self.transfer.paste_ok_items = 0;
        self.transfer.paste_failed_items = 0;
        self.copy.total_rx = Some(spawn_worker(move |tx_total| {
            let total = sources
                .iter()
                .filter_map(|src| App::compute_total_bytes(src).ok())
                .fold(0u64, |acc, v| acc.saturating_add(v));
            let _ = tx_total.send(total);
        }));
        self.copy.total_bytes = 0;
        self.copy.done_bytes = 0;
        self.copy.done_before_job = 0;
        self.copy.job_total_bytes = 0;
        self.copy.started_at = Some(Instant::now());
        self.copy.current_src = None;
        self.advance_paste_queue();
    }

    pub(crate) fn begin_transfer(&mut self, move_mode: bool) {
        if self.clipboard.is_empty() {
            self.set_status("clipboard is empty");
            return;
        }
        self.begin_transfer_from_sources(self.clipboard.clone(), self.left.dir.clone(), move_mode);
    }

    pub(crate) fn begin_dual_panel_transfer(&mut self, move_mode: bool) {
        if !self.is_dual_panel_mode() {
            self.set_status("dual panel mode is not active");
            return;
        }

        let (sources, target_dir) = match self.active_panel {
            DualPanelSide::Left => {
                let sources = if !self.left.marked_indices.is_empty() {
                    self.left.entries
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| self.left.marked_indices.contains(i))
                        .map(|(_, e)| e.path())
                        .collect()
                } else {
                    self.left.entries
                        .get(self.left.selected_index)
                        .map(|e| vec![e.path()])
                        .unwrap_or_default()
                };
                (sources, self.right.dir.clone())
            }
            DualPanelSide::Right => {
                let sources = if !self.right.marked_indices.is_empty() {
                    self.right.entries
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| self.right.marked_indices.contains(i))
                        .map(|(_, e)| e.path())
                        .collect()
                } else {
                    self.right.entries
                        .get(self.right.selected_index)
                        .map(|e| vec![e.path()])
                        .unwrap_or_default()
                };
                (sources, self.left.dir.clone())
            }
        };

        self.begin_transfer_from_sources(sources, target_dir, move_mode);
    }

    pub(crate) fn pump_copy_total_prescan(&mut self) {
        if let Some(total) = pump_once(&mut self.copy.total_rx) {
            self.copy.total_bytes = total;
        }
    }

    pub(crate) fn begin_paste(&mut self) {
        self.begin_transfer(false);
    }

    pub(crate) fn begin_move(&mut self) {
        self.begin_transfer(true);
    }

    pub(crate) fn copy_full_paths_to_system_clipboard(&mut self) {
        let targets = self.delete_targets();
        if targets.is_empty() {
            self.set_status("no selected item");
            return;
        }

        let payload = targets
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");

        match self.write_system_clipboard_text(&payload) {
            Some(backend) => self.set_status(format!(
                "copied {} full path(s) to system clipboard via {}",
                targets.len(),
                backend
            )),
            None => {
                self.set_status("no clipboard backend available (wl-copy/xclip/xsel/pbcopy)")
            }
        }
    }

    pub(crate) fn read_system_clipboard_text(&self) -> Option<(String, &'static str)> {
        for backend in ["wl-copy", "xclip", "xsel", "pbcopy"] {
            if !self.integration_active(backend) {
                continue;
            }

            let output = match backend {
                "wl-copy" => {
                    if !Self::integration_probe("wl-paste").0 {
                        continue;
                    }
                    Command::new("wl-paste")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output()
                }
                "xclip" => Command::new("xclip")
                    .args(["-selection", "clipboard", "-out"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output(),
                "xsel" => Command::new("xsel")
                    .args(["--clipboard", "--output"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output(),
                "pbcopy" => {
                    if !Self::integration_probe("pbpaste").0 {
                        continue;
                    }
                    Command::new("pbpaste")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output()
                }
                _ => continue,
            };

            if let Ok(out) = output
                && out.status.success() {
                    return Some((String::from_utf8_lossy(&out.stdout).into_owned(), backend));
                }
        }

        None
    }

    pub(crate) fn write_system_clipboard_text(&self, payload: &str) -> Option<&'static str> {
        for backend in ["wl-copy", "xclip", "xsel", "pbcopy"] {
            if !self.integration_active(backend) {
                continue;
            }

            let mut cmd = match backend {
                "wl-copy" => Command::new("wl-copy"),
                "xclip" => {
                    let mut cmd = Command::new("xclip");
                    cmd.args(["-selection", "clipboard"]);
                    cmd
                }
                "xsel" => {
                    let mut cmd = Command::new("xsel");
                    cmd.args(["--clipboard", "--input"]);
                    cmd
                }
                "pbcopy" => Command::new("pbcopy"),
                _ => continue,
            };

            let mut child = match cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => continue,
            };

            let write_ok = child
                .stdin
                .take()
                .map(|mut stdin| stdin.write_all(payload.as_bytes()).is_ok())
                .unwrap_or(false);
            if !write_ok {
                let _ = child.kill();
                let _ = child.wait();
                continue;
            }

            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Some(backend);
            }
        }

        None
    }

    pub(crate) fn edit_system_clipboard_via_temp_file(&mut self) -> io::Result<()> {
        let Some((clipboard_text, read_backend)) = self.read_system_clipboard_text() else {
            self.set_status("no clipboard backend available (wl-copy/xclip/xsel/pbcopy)");
            return Ok(());
        };

        let tmp = Self::create_temp_selection_path("sbrs_clipboard_edit");
        if fs::write(&tmp, clipboard_text.as_bytes()).is_err() {
            self.set_status("failed to create temporary clipboard file");
            return Ok(());
        }

        let edit_result = {
            let _tui = tui::suspend_showing_cursor(ResumeMode::Plain)?;
            let _ = Command::new(crate::util::command::editor_command())
                .arg(&tmp)
                .status();
            fs::read_to_string(&tmp)
        };

        let _ = fs::remove_file(&tmp);

        match edit_result {
            Ok(updated_text) => {
                if let Some(write_backend) = self.write_system_clipboard_text(&updated_text) {
                    self.set_status(format!(
                        "clipboard updated via {} (read via {})",
                        write_backend, read_backend
                    ));
                } else {
                    self.set_status("failed to write updated clipboard content");
                }
            }
            Err(e) => {
                self.set_status(format!("clipboard edit failed: {}", e));
            }
        }

        Ok(())
    }

    pub(crate) fn copy_path_with_progress(
        src: &PathBuf,
        dest: &PathBuf,
        tx: &Sender<CopyProgressMsg>,
        copied_bytes: &mut u64,
    ) -> io::Result<()> {
        if src.is_dir() {
            fs::create_dir_all(dest)?;
            for child in fs::read_dir(src)? {
                let child = child?;
                let child_src = child.path();
                let child_dest = dest.join(child.file_name());
                Self::copy_path_with_progress(&child_src, &child_dest, tx, copied_bytes)?;
            }
            Ok(())
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut in_file = fs::File::open(src)?;
            let mut out_file = fs::File::create(dest)?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = in_file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                out_file.write_all(&buffer[..read])?;
                *copied_bytes = copied_bytes.saturating_add(read as u64);
                let _ = tx.send(CopyProgressMsg::CopiedBytes(*copied_bytes));
            }
            Ok(())
        }
    }

    pub(crate) fn update_copy_status(&mut self) {
        if self.copy.item_name.is_empty() {
            return;
        }
        let total = self.copy.total_bytes;
        let scanning = total == 0 && self.copy.total_rx.is_some();
        let done = if total == 0 {
            self.copy.done_bytes
        } else {
            self.copy.done_bytes.min(total)
        };
        let effective_total = if total == 0 {
            done
                .saturating_add(self.copy.job_total_bytes)
                .max(1)
        } else {
            total.max(1)
        };
        let percent = if total == 0 {
            if self.copy.total_rx.is_some() { 0.0 } else { 100.0 }
        } else {
            (done as f64 * 100.0) / effective_total as f64
        };
        let elapsed_secs = self
            .copy.started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
            .max(0.001);
        let bytes_per_sec = done as f64 / elapsed_secs;
        let remaining = if total == 0 { 0 } else { total.saturating_sub(done) };
        let eta_secs = if bytes_per_sec > 0.0 {
            (remaining as f64 / bytes_per_sec) as u64
        } else {
            0
        };
        let bar = crate::util::format::progress_bar(percent, 14);
        let total_label = if total == 0 && self.copy.total_rx.is_some() {
            "?".to_string()
        } else {
            Self::format_size(effective_total)
        };
        let eta_label = if total == 0 { "-".to_string() } else { Self::format_eta(eta_secs) };
        let scan_suffix = if scanning { " scanning size..." } else { "" };
        let current_idx = (self.transfer.paste_ok_items + self.transfer.paste_failed_items + 1).min(self.transfer.paste_total_items.max(1));
        let scope = if self.copy.from_remote { "remote " } else { "" };
        self.set_status(format!(
            "{}copy [{}] {:>3.0}% {}/{} {}/s eta {} ({}/{}) {}{}",
            scope,
            bar,
            percent,
            Self::format_size(done),
            total_label,
            Self::format_size(bytes_per_sec as u64),
            eta_label,
            current_idx,
            self.transfer.paste_total_items,
            self.copy.item_name,
            scan_suffix
        ));
    }

    pub(crate) fn start_copy_job(&mut self, src: PathBuf, dest: PathBuf, display_name: String) {
        self.copy.done_before_job = self.copy.done_bytes;
        self.copy.job_total_bytes = 0;
        self.copy.item_name = display_name;
        self.copy.current_src = Some(src.clone());
        self.copy.from_remote = self.is_path_inside_remote_mount(&src);
        self.update_copy_status();

        self.copy.rx = Some(spawn_worker(move |tx| {
            let total = Self::compute_total_bytes(&src).unwrap_or(0);
            let _ = tx.send(CopyProgressMsg::TotalBytes(total));
            let mut copied = 0u64;
            let result = Self::copy_path_with_progress(&src, &dest, &tx, &mut copied)
                .map_err(|e| e.to_string());
            let _ = tx.send(CopyProgressMsg::Finished(result));
        }));
    }

    pub(crate) fn pump_copy_progress(&mut self) {
        if self.copy.rx.is_none() {
            return;
        }

        let mut done_result: Option<Result<(), String>> = None;
        for msg in drain_channel(&mut self.copy.rx) {
            match msg {
                CopyProgressMsg::TotalBytes(total) => {
                    self.copy.job_total_bytes = total;
                }
                CopyProgressMsg::CopiedBytes(done) => {
                    self.copy.done_bytes = self.copy.done_before_job.saturating_add(done);
                }
                CopyProgressMsg::Finished(result) => {
                    done_result = Some(result);
                }
            }
        }
        // Sender dropped without a `Finished` message: the worker died.
        if done_result.is_none() && self.copy.rx.is_none() {
            done_result = Some(Err("copy worker disconnected".to_string()));
        }

        if let Some(result) = done_result {
            self.copy.rx = None;
            match result {
                Ok(()) => {
                    if self.transfer.paste_move_mode
                        && let Some(src) = self.copy.current_src.take() {
                            let delete_res = if src.is_dir() {
                                fs::remove_dir_all(&src)
                            } else {
                                fs::remove_file(&src)
                            };
                            if let Err(e) = delete_res {
                                self.transfer.paste_failed_items += 1;
                                self.set_status(format!("move cleanup failed for {}: {}", self.copy.item_name, e));
                                self.copy.job_total_bytes = 0;
                                self.copy.done_before_job = self.copy.done_bytes;
                                self.copy.item_name.clear();
                                self.copy.from_remote = false;
                                let _ = self.refresh_entries();
                                if self.is_dual_panel_mode() {
                                    let _ = self.refresh_right_panel_entries();
                                }
                                self.advance_paste_queue();
                                return;
                            }
                        }
                    self.transfer.paste_ok_items += 1;
                    self.copy.done_bytes = self
                        .copy.done_before_job
                        .saturating_add(self.copy.job_total_bytes);
                }
                Err(e) => {
                    self.transfer.paste_failed_items += 1;
                    self.set_status(format!("paste failed for {}: {}", self.copy.item_name, e));
                }
            }
            self.copy.job_total_bytes = 0;
            self.copy.done_before_job = self.copy.done_bytes;
            self.copy.item_name.clear();
            self.copy.current_src = None;
            self.copy.from_remote = false;
            let _ = self.refresh_entries();
            if self.is_dual_panel_mode() {
                let _ = self.refresh_right_panel_entries();
            }
            self.advance_paste_queue();
        } else {
            self.update_copy_status();
        }
    }

    pub(crate) fn format_eta(total_seconds: u64) -> String {
        util::format::format_eta(total_seconds)
    }

    pub(crate) fn advance_paste_queue(&mut self) {
        if self.copy.rx.is_some() {
            return;
        }
        while let Some(src) = self.transfer.paste_queue.pop_front() {
            let name = src
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "pasted_item".to_string());
            let target_dir = self
                .transfer.paste_target_dir
                .as_ref()
                .cloned()
                .unwrap_or_else(|| self.left.dir.clone());
            let dest = target_dir.join(&name);
            if dest.exists() {
                self.transfer.paste_current_src = Some(src);
                self.begin_input_edit(AppMode::PasteRenaming, name);
                self.set_status("target exists: edit name and press Enter");
                return;
            }

            if self.transfer.paste_move_mode
                && fs::rename(&src, &dest).is_ok() {
                    self.transfer.paste_ok_items += 1;
                    let _ = self.refresh_entries();
                    if self.is_dual_panel_mode() {
                        let _ = self.refresh_right_panel_entries();
                    }
                    continue;
                }

            self.start_copy_job(src, dest, name);
            return;
        }

        self.transfer.paste_current_src = None;
        self.transfer.paste_move_mode = false;
        self.transfer.paste_target_dir = None;
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
        self.copy.started_at = None;
        self.copy.total_rx = None;
        self.copy.current_src = None;
        self.refresh_entries_or_status();
        if self.is_dual_panel_mode() {
            let _ = self.refresh_right_panel_entries();
        }
        if self.transfer.paste_failed_items == 0 && self.transfer.paste_ok_items > 0 {
            self.set_status(format!("transfer complete: {} item", self.transfer.paste_ok_items));
        } else if self.transfer.paste_failed_items == 0 {
            self.set_status("nothing to transfer");
        } else {
            self.set_status(format!(
                "transfer finished: {} ok, {} failed ({} total)",
                self.transfer.paste_ok_items, self.transfer.paste_failed_items, self.transfer.paste_total_items
            ));
        }
    }
}
