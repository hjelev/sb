use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::enable_raw_mode,
};

use crate::util::tui::{resume_tui, resume_tui_cleared, suspend_tui};
use crate::{App, DualPanelSide};

impl App {
    /// The user's login shell from `$SHELL`, falling back to `/bin/sh`.
    fn login_shell() -> String {
        env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }

    pub(crate) fn shell_single_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    /// Pick the terminal multiplexer to drive split shells: tmux is preferred,
    /// zellij is the fallback when tmux isn't available.
    fn split_shell_multiplexer(&self) -> Option<&'static str> {
        if self.integration_active("tmux") {
            Some("tmux")
        } else if self.integration_active("zellij") {
            Some("zellij")
        } else {
            None
        }
    }

    pub(crate) fn open_split_shell_with_less(&mut self) -> io::Result<()> {
        let Some(mux) = self.split_shell_multiplexer() else {
            self.status_tool_not_found("tmux/zellij");
            return Ok(());
        };

        let Some(entry) = self.entries.get(self.selected_index) else {
            self.set_status("no selected item");
            return Ok(());
        };

        let selected_path = entry.path();
        if selected_path.is_dir() {
            self.set_status("split shell preview works on files only");
            return Ok(());
        }

        let shell = Self::login_shell();
        let current_dir = self.current_dir.to_string_lossy().into_owned();
        let selected_file = selected_path.to_string_lossy().into_owned();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let session_name = format!("sbrs_i_{}_{}", std::process::id(), stamp % 1_000_000_000);
        let right_cmd = format!("less -R -- {}", Self::shell_single_quote(&selected_file));

        self.exec_split(mux, &session_name, &shell, &current_dir, &right_cmd)
    }

    pub(crate) fn open_split_shell_with_editor(&mut self) -> io::Result<()> {
        let Some(mux) = self.split_shell_multiplexer() else {
            self.status_tool_not_found("tmux/zellij");
            return Ok(());
        };

        let Some(entry) = self.entries.get(self.selected_index) else {
            self.set_status("no selected item");
            return Ok(());
        };

        let selected_path = entry.path();
        if selected_path.is_dir() {
            self.set_status("split shell edit works on files only");
            return Ok(());
        }

        let shell = Self::login_shell();
        let editor = crate::util::command::editor_command();
        let current_dir = self.current_dir.to_string_lossy().into_owned();
        let selected_file = selected_path.to_string_lossy().into_owned();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let session_name = format!("sbrs_E_{}_{}", std::process::id(), stamp % 1_000_000_000);
        let right_cmd = format!("{} -- {}", editor, Self::shell_single_quote(&selected_file));

        self.exec_split(mux, &session_name, &shell, &current_dir, &right_cmd)
    }

    /// Suspend the TUI, drive the chosen multiplexer to show a shell on the
    /// left and `right_cmd` on the right, then resume the TUI.
    fn exec_split(
        &mut self,
        mux: &str,
        session_name: &str,
        shell: &str,
        current_dir: &str,
        right_cmd: &str,
    ) -> io::Result<()> {
        suspend_tui()?;
        execute!(io::stdout(), Show)?;

        let result = match mux {
            "zellij" => Self::run_zellij_split(session_name, shell, current_dir, right_cmd),
            _ => Self::run_tmux_split(session_name, shell, current_dir, right_cmd),
        };

        resume_tui_cleared()?;
        enable_raw_mode()?;
        execute!(io::stdout(), Hide)?;

        match result {
            Ok(()) => self.set_status("returned from split shell"),
            Err(e) => self.set_status(format!("split shell failed: {}", e)),
        }
        self.refresh_entries_or_status();
        Ok(())
    }

    fn run_tmux_split(
        session_name: &str,
        shell: &str,
        current_dir: &str,
        right_cmd: &str,
    ) -> io::Result<()> {
        let left_cmd = format!(
            "{} -i; tmux kill-session -t {} >/dev/null 2>&1",
            Self::shell_single_quote(shell),
            Self::shell_single_quote(session_name)
        );
        let target_window = format!("{}:0", session_name);
        let target_left = format!("{}:0.0", session_name);

        let create_status = Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name, "-c", current_dir, &left_cmd])
            .status()?;
        if !create_status.success() {
            return Err(io::Error::other("tmux new-session failed"));
        }

        let split_status = Command::new("tmux")
            .args(["split-window", "-h", "-p", "30", "-t", &target_window, "-c", current_dir, right_cmd])
            .status()?;
        if !split_status.success() {
            let _ = Command::new("tmux").args(["kill-session", "-t", session_name]).status();
            return Err(io::Error::other("tmux split-window failed"));
        }

        let _ = Command::new("tmux")
            .args(["select-pane", "-t", &target_left])
            .status();

        let _ = Command::new("tmux")
            .args(["attach-session", "-t", session_name])
            .status();

        let _ = Command::new("tmux")
            .args(["kill-session", "-t", session_name])
            .status();

        Ok(())
    }

    fn run_zellij_split(
        session_name: &str,
        shell: &str,
        current_dir: &str,
        right_cmd: &str,
    ) -> io::Result<()> {
        // zellij has no scriptable session/split API like tmux, so describe the
        // split as a temporary KDL layout. Both panes close on exit so quitting
        // the viewer/editor and exiting the shell returns to sbrs.
        let layout = format!(
            "layout {{\n    \
                 pane split_direction=\"vertical\" {{\n        \
                     pane {{\n            \
                         command \"{shell}\"\n            \
                         args \"-i\"\n            \
                         cwd \"{cwd}\"\n            \
                         close_on_exit true\n        \
                     }}\n        \
                     pane size=\"30%\" {{\n            \
                         command \"{shell}\"\n            \
                         args \"-c\" \"{right}\"\n            \
                         cwd \"{cwd}\"\n            \
                         close_on_exit true\n        \
                     }}\n    \
                 }}\n    \
                 pane size=2 borderless=true {{\n        \
                     plugin location=\"zellij:status-bar\"\n    \
                 }}\n}}\n",
            shell = Self::kdl_escape(shell),
            cwd = Self::kdl_escape(current_dir),
            right = Self::kdl_escape(right_cmd),
        );

        let layout_path = env::temp_dir().join(format!("{}.kdl", session_name));
        std::fs::write(&layout_path, layout)?;

        // `--session NAME --layout FILE` would try to add a tab to an existing
        // session; `--new-session-with-layout` always starts a fresh session.
        let status = Command::new("zellij")
            .args(["--session", session_name, "--new-session-with-layout"])
            .arg(&layout_path)
            .status();

        let _ = Command::new("zellij")
            .args(["delete-session", session_name, "--force"])
            .status();
        let _ = std::fs::remove_file(&layout_path);

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(_) => Err(io::Error::other("zellij exited with failure")),
            Err(e) => Err(e),
        }
    }

    /// Escape a string for embedding inside a KDL double-quoted value.
    fn kdl_escape(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    pub(crate) fn run_shell_command_and_wait_key(&mut self, command: &str) -> io::Result<()> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            self.set_status("command cancelled");
            return Ok(());
        }

        suspend_tui()?;

        println!("$ {}", trimmed);
        let shell = Self::login_shell();
        let mut cmd = Command::new(&shell);
        // Non-interactive mode avoids shell job-control side effects that can
        // suspend sbrs when returning from the command runner.
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

        println!("\nPress Enter to return to shell buddy...");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);

        resume_tui_cleared()?;
        enable_raw_mode()?;

        self.set_status(format!("ran command: {}", trimmed));
        self.refresh_entries_or_status();
        Ok(())
    }

    pub(crate) fn drop_to_shell(&mut self) -> io::Result<()> {
        let shell = Self::login_shell();
        suspend_tui()?;
        execute!(io::stdout(), Show)?;
        let _ = Command::new(&shell)
            .current_dir(&self.current_dir)
            .status();
        resume_tui_cleared()?;
        enable_raw_mode()?;
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
                let mut cmd = Command::new("mmdflux");
                cmd.arg(path);
                let _ = crate::util::command::pipe_to_pager(cmd);
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
                let mut cmd = Command::new("pdftotext");
                cmd.args(["-layout", "-nopgbrk"]).arg(path).arg("-");
                let shown = crate::util::command::pipe_to_pager(cmd);
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
                    if let Some(out) = child.stdout {
                        let _ = Command::new("less")
                            .args(["-R"])
                            .stdin(out)
                            .status();
                    }
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
            self.status_tool_not_found("delta");
            return Ok(());
        }

        if self.marked_indices.len() != 1 {
            self.set_status("mark exactly one file, then move cursor to another file and press C");
            return Ok(());
        }

        let marked_idx = *self.marked_indices.iter().next().unwrap_or(&self.selected_index);
        let Some(marked_entry) = self.entries.get(marked_idx) else {
            self.set_status("marked file not found");
            return Ok(());
        };
        let Some(cursor_entry) = self.entries.get(self.selected_index) else {
            self.set_status("cursor file not found");
            return Ok(());
        };

        let marked_path = marked_entry.path();
        let cursor_path = cursor_entry.path();

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

        let left = crate::util::classify::display_name(marked_path.as_path());
        let right = crate::util::classify::display_name(cursor_path.as_path());
        self.set_status(format!("delta compared: {} vs {}", left, right));
        Ok(())
    }

    pub(crate) fn open_selected_with_default_app(&mut self) -> io::Result<()> {
        let entry = if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.entries.get(self.right.selected_index)
        } else {
            self.entries.get(self.selected_index)
        };
        let Some(entry) = entry else {
            self.set_status("no selected item");
            return Ok(());
        };

        let path = entry.path();
        let display_name = crate::util::classify::display_name(path.as_path());

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
