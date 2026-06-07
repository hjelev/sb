use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{Clear as TermClear, ClearType},
};
use crate::util::tui::{suspend_tui, resume_tui};

use crate::App;

impl App {
    pub(crate) fn shell_single_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    pub(crate) fn open_split_shell_with_less(&mut self) -> io::Result<()> {
        if !self.integration_active("tmux") {
            self.set_status("tmux not found in PATH");
            return Ok(());
        }

        let Some(selected_path) = self.active_selected_entry_path() else {
            self.set_status("no selected item");
            return Ok(());
        };

        if selected_path.is_dir() {
            self.set_status("split shell preview works on files only");
            return Ok(());
        }

        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let current_dir = self.active_panel_dir().to_string_lossy().into_owned();
        let selected_file = selected_path.to_string_lossy().into_owned();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let session_name = format!("sbrs_i_{}_{}", std::process::id(), stamp % 1_000_000_000);

        suspend_tui()?;
        execute!(io::stdout(), Show)?;

        let tmux_result = (|| -> io::Result<()> {
            let left_cmd = format!(
                "{} -i; tmux kill-session -t {} >/dev/null 2>&1",
                Self::shell_single_quote(&shell),
                Self::shell_single_quote(&session_name)
            );
            let right_cmd = format!("less -R -- {}", Self::shell_single_quote(&selected_file));
            let target_window = format!("{}:0", session_name);
            let target_left = format!("{}:0.0", session_name);

            let create_status = Command::new("tmux")
                .args([
                    "new-session",
                    "-d",
                    "-s",
                    &session_name,
                    "-c",
                    &current_dir,
                    &left_cmd,
                ])
                .status()?;
            if !create_status.success() {
                return Err(io::Error::other("tmux new-session failed"));
            }

            let split_status = Command::new("tmux")
                .args([
                    "split-window",
                    "-h",
                    "-p",
                    "30",
                    "-t",
                    &target_window,
                    "-c",
                    &current_dir,
                    &right_cmd,
                ])
                .status()?;
            if !split_status.success() {
                let _ = Command::new("tmux")
                    .args(["kill-session", "-t", &session_name])
                    .status();
                return Err(io::Error::other("tmux split-window failed"));
            }

            let _ = Command::new("tmux")
                .args(["select-pane", "-t", &target_left])
                .status();

            let _ = Command::new("tmux")
                .args(["attach-session", "-t", &session_name])
                .status();

            let _ = Command::new("tmux")
                .args(["kill-session", "-t", &session_name])
                .status();

            Ok(())
        })();

        resume_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        execute!(io::stdout(), Hide)?;

        match tmux_result {
            Ok(()) => self.set_status("returned from split shell"),
            Err(e) => self.set_status(format!("split shell failed: {}", e)),
        }
        self.refresh_entries_or_status();
        Ok(())
    }

    pub(crate) fn open_split_shell_with_editor(&mut self) -> io::Result<()> {
        if !self.integration_active("tmux") {
            self.set_status("tmux not found in PATH");
            return Ok(());
        }

        let Some(selected_path) = self.active_selected_entry_path() else {
            self.set_status("no selected item");
            return Ok(());
        };

        if selected_path.is_dir() {
            self.set_status("split shell edit works on files only");
            return Ok(());
        }

        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let editor = env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
        let current_dir = self.active_panel_dir().to_string_lossy().into_owned();
        let selected_file = selected_path.to_string_lossy().into_owned();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let session_name = format!("sbrs_E_{}_{}", std::process::id(), stamp % 1_000_000_000);

        suspend_tui()?;
        execute!(io::stdout(), Show)?;

        let tmux_result = (|| -> io::Result<()> {
            let left_cmd = format!(
                "{} -i; tmux kill-session -t {} >/dev/null 2>&1",
                Self::shell_single_quote(&shell),
                Self::shell_single_quote(&session_name)
            );
            let right_cmd = format!("{} -- {}", editor, Self::shell_single_quote(&selected_file));
            let target_window = format!("{}:0", session_name);
            let target_left = format!("{}:0.0", session_name);

            let create_status = Command::new("tmux")
                .args([
                    "new-session",
                    "-d",
                    "-s",
                    &session_name,
                    "-c",
                    &current_dir,
                    &left_cmd,
                ])
                .status()?;
            if !create_status.success() {
                return Err(io::Error::other("tmux new-session failed"));
            }

            let split_status = Command::new("tmux")
                .args([
                    "split-window",
                    "-h",
                    "-p",
                    "30",
                    "-t",
                    &target_window,
                    "-c",
                    &current_dir,
                    &right_cmd,
                ])
                .status()?;
            if !split_status.success() {
                let _ = Command::new("tmux")
                    .args(["kill-session", "-t", &session_name])
                    .status();
                return Err(io::Error::other("tmux split-window failed"));
            }

            let _ = Command::new("tmux")
                .args(["select-pane", "-t", &target_left])
                .status();

            let _ = Command::new("tmux")
                .args(["attach-session", "-t", &session_name])
                .status();

            let _ = Command::new("tmux")
                .args(["kill-session", "-t", &session_name])
                .status();

            Ok(())
        })();

        resume_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        execute!(io::stdout(), Hide)?;

        match tmux_result {
            Ok(()) => self.set_status("returned from split shell"),
            Err(e) => self.set_status(format!("split shell failed: {}", e)),
        }
        self.refresh_entries_or_status();
        Ok(())
    }

    pub(crate) fn run_shell_command_and_wait_key(&mut self, command: &str) -> io::Result<()> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            self.set_status("command cancelled");
            return Ok(());
        }

        suspend_tui()?;

        println!("$ {}", trimmed);
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = Command::new(&shell);
        cmd.args(["-c", trimmed]);

        let status = cmd.current_dir(&self.current_dir).status();

        match status {
            Ok(s) => {
                if let Some(code) = s.code() {
                    println!("\n[exit code: {}]", code);
                } else {
                    println!("\n[process terminated by signal]");
                }
            }
            Err(e) => {
                println!("\n[failed to execute command: {}]", e);
            }
        }

        println!("\nPress Enter to return to sbrs...");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);

        resume_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;

        self.set_status(format!("ran command: {}", trimmed));
        self.refresh_entries_or_status();
        Ok(())
    }

    pub(crate) fn drop_to_shell(&mut self) -> io::Result<()> {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        suspend_tui()?;
        execute!(io::stdout(), Show)?;
        let _ = Command::new(&shell)
            .current_dir(&self.current_dir)
            .status();
        resume_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        execute!(io::stdout(), Hide)?;
        self.set_status("returned from shell");
        self.refresh_entries_or_status();
        Ok(())
    }

    pub(crate) fn open_path_in_view_mode(path: &PathBuf, use_pager: bool) -> io::Result<()> {
        if Self::is_image_file(path) {
            if Self::integration_probe("viu").0 {
                let _ = Command::new("viu").arg(path).status();
                return Ok(());
            }
            if Self::integration_probe("chafa").0 {
                let _ = Command::new("chafa").arg(path).status();
                return Ok(());
            }
        }

        if Self::is_markdown_file(path) && Self::integration_probe("glow").0 {
            let mut cmd = Command::new("glow");
            if use_pager {
                cmd.arg("-p");
            }
            let _ = cmd.arg(path).status();
            return Ok(());
        }

        if Self::is_mermaid_file(path) && Self::integration_probe("mmdflux").0 {
            if use_pager {
                if let Ok(mut child) = Command::new("mmdflux")
                    .arg(path)
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    if let Some(mmd_out) = child.stdout.take() {
                        let _ = Command::new("less").args(["-R"]).stdin(mmd_out).status();
                    }
                    let _ = child.wait();
                }
            } else {
                let _ = Command::new("mmdflux").arg(path).status();
            }
            return Ok(());
        }

        if Self::is_html_file(path) && Self::integration_probe("links").0 {
            let _ = Command::new("links").arg(path).status();
            return Ok(());
        }

        if Self::is_json_file(path) && Self::integration_probe("jnv").0 {
            let _ = Command::new("jnv").arg(path).status();
            return Ok(());
        }

        if Self::is_delimited_text_file(path) && Self::integration_probe("csvlens").0 {
            let _ = Command::new("csvlens").arg(path).status();
            return Ok(());
        }

        if Self::is_audio_file(path) && Self::integration_probe("sox").0 {
            if Self::integration_probe("play").0 {
                let _ = Command::new("play")
                    .arg(path)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            } else {
                let _ = Command::new("sox")
                    .arg(path)
                    .arg("-d")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            return Ok(());
        }

        if Self::is_pdf_file(path) && Self::integration_probe("pdftotext").0 {
            if use_pager {
                let mut shown = false;
                if let Ok(mut child) = Command::new("pdftotext")
                    .args(["-layout", "-nopgbrk"])
                    .arg(path)
                    .arg("-")
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    if let Some(pdf_text) = child.stdout.take() {
                        shown = Command::new("less")
                            .args(["-R"])
                            .stdin(pdf_text)
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false);
                    }
                    let _ = child.wait();
                }
                if !shown {
                    let _ = Command::new("less")
                        .args(["-R", path.to_str().unwrap_or_default()])
                        .status();
                }
            } else {
                let _ = Command::new("pdftotext")
                    .args(["-layout", "-nopgbrk"])
                    .arg(path)
                    .arg("-")
                    .status();
            }
            return Ok(());
        }

        if Self::is_cast_file(path) && Self::integration_probe("asciinema").0 {
            let _ = Command::new("asciinema").args(["play", "-i"]).arg(path).status();
            return Ok(());
        }

        if Self::is_binary_file(path) && Self::integration_probe("hexyl").0 {
            if use_pager {
                if let Ok(child) = Command::new("hexyl")
                    .arg(path)
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    let _ = Command::new("less")
                        .args(["-R"])
                        .stdin(child.stdout.unwrap())
                        .status();
                    return Ok(());
                }
            } else {
                let _ = Command::new("hexyl").arg(path).status();
                return Ok(());
            }
        }

        if Self::integration_probe("bat").0 {
            let bat_cmd = Self::bat_tool().unwrap_or_else(|| "bat".to_string());
            let paging = if use_pager { "always" } else { "never" };
            let _ = Command::new(bat_cmd)
                .args([&format!("--paging={}", paging), "--style=full", "--color=always"])
                .arg(path)
                .status();
            return Ok(());
        }

        if use_pager {
            let _ = Command::new("less")
                .args(["-R", path.to_str().unwrap_or_default()])
                .status();
        } else {
            let _ = Command::new("cat")
                .arg(path)
                .status();
        }
        Ok(())
    }

    pub(crate) fn run_delta_compare(&mut self) -> io::Result<()> {
        if !self.integration_active("delta") {
            self.set_status("delta not found in PATH");
            return Ok(());
        }

        if self.marked_indices.len() != 1 {
            self.set_status("mark exactly one file, then move cursor to another file and press C");
            return Ok(());
        }

        let (entries, marked_indices, selected_index) =
            if self.is_dual_panel_mode() && self.active_panel == crate::DualPanelSide::Right {
                (&self.right.entries, &self.right.marked_indices, self.right.selected_index)
            } else {
                (&self.entries, &self.marked_indices, self.selected_index)
            };

        let marked_idx = *marked_indices.iter().next().unwrap_or(&selected_index);
        let Some(marked_path) = entries.get(marked_idx).map(|e| e.path()) else {
            self.set_status("marked file not found");
            return Ok(());
        };
        let Some(cursor_path) = entries.get(selected_index).map(|e| e.path()) else {
            self.set_status("cursor file not found");
            return Ok(());
        };

        if marked_path == cursor_path {
            self.set_status("choose a different cursor file to compare");
            return Ok(());
        }
        if marked_path.is_dir() || cursor_path.is_dir() {
            self.set_status("delta compare works on files only");
            return Ok(());
        }

        suspend_tui()?;
        let _ = Command::new("delta")
            .arg("--side-by-side")
            .arg("--paging=always")
            .arg(&marked_path)
            .arg(&cursor_path)
            .status();
        resume_tui()?;

        let left = marked_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| marked_path.to_string_lossy().into_owned());
        let right = cursor_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| cursor_path.to_string_lossy().into_owned());
        self.set_status(format!("delta compared: {} vs {}", left, right));
        Ok(())
    }

    pub(crate) fn open_selected_with_default_app(&mut self) -> io::Result<()> {
        let Some(path) = self.active_selected_entry_path() else {
            self.set_status("no selected item");
            return Ok(());
        };
        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        #[cfg(target_os = "macos")]
        let opened = if Self::integration_probe("open").0 {
            Command::new("open")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        } else {
            false
        };

        #[cfg(not(target_os = "macos"))]
        let opened = if Self::integration_probe("xdg-open").0 {
            Command::new("xdg-open")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        } else if Self::integration_probe("gio").0 {
            Command::new("gio")
                .arg("open")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        } else {
            false
        };

        if opened {
            self.set_status(format!("opened with default app: {}", display_name));
        } else {
            #[cfg(target_os = "macos")]
            self.set_status("no default opener found (tried open)");

            #[cfg(not(target_os = "macos"))]
            self.set_status("no default opener found (tried xdg-open, gio open)");
        }

        Ok(())
    }
}
