//! Command execution builder pattern.
//!
//! Replaces scattered `Command::new()` patterns with a builder that:
//! - Captures stderr for better error messages
//! - Returns consistent Result types (no silent failures)
//! - Provides high-level methods for common commands (git, archive, preview)

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Spawn `cmd` with its stdout piped into `less -R`, returning whether the pager
/// exited successfully. The spawned child is reaped after the pager closes.
///
/// Centralizes the repeated "spawn preview tool → pipe into `less -R`" blocks
/// (mermaid/`mmdflux`, `pdftotext`, `hexyl`, …). Callers that don't care about
/// success use `let _ = pipe_to_pager(cmd);`.
pub fn pipe_to_pager(mut cmd: Command) -> bool {
    let Ok(mut child) = cmd.stdout(Stdio::piped()).spawn() else {
        return false;
    };
    let mut shown = false;
    if let Some(out) = child.stdout.take() {
        shown = Command::new("less")
            .args(["-R"])
            .stdin(out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    let _ = child.wait();
    shown
}

/// Spawn `git <args>` in `cwd` with its stdout piped into `tool`'s stdin,
/// leaving the terminal otherwise inherited so stdin-fed diff pagers like
/// `diffnav` can take over the screen. Returns whether the pager ran and
/// exited successfully, so callers can fall back to another diff viewer.
pub fn pipe_git_to_tool<P: AsRef<Path>>(cwd: P, git_args: &[&str], tool: &str, tool_args: &[&str]) -> bool {
    let Ok(mut git) = Command::new("git")
        .args(git_args)
        .current_dir(cwd.as_ref())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    else {
        return false;
    };
    let mut shown = false;
    if let Some(out) = git.stdout.take() {
        shown = Command::new(tool)
            .args(tool_args)
            .stdin(out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    let _ = git.wait();
    shown
}

/// Page `cmd`'s output through `less -R`, falling back to paging `path` directly
/// when the tool can't be spawned.
///
/// Centralizes the "run viewer → pipe into less → otherwise just `less` the
/// file" idiom repeated by the hexyl / pdftotext viewers.
pub fn pipe_to_pager_or_less(cmd: Command, path: &Path) {
    if !pipe_to_pager(cmd) {
        let _ = Command::new("less").arg("-R").arg(path).status();
    }
}

/// Spawn `program path [args…]` fully detached (all stdio set to null) and
/// return the [`Child`] so the caller can wait on or kill it.
///
/// Centralizes the duplicated null-stdio spawn used by the audio players
/// (`play` / `sox`).
pub fn spawn_detached(program: &str, path: &Path, args: &[&str]) -> io::Result<Child> {
    Command::new(program)
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// The user's preferred terminal editor program.
///
/// Reads `$EDITOR`, falling back to `nano`. Centralizes the
/// `env::var("EDITOR").unwrap_or_else(|_| "nano".to_string())` pattern that was
/// duplicated across the editor/notes/transfer launch sites.
pub fn editor_command() -> String {
    std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string())
}

/// Builder for executing external commands with consistent error handling.
pub struct CommandBuilder;

impl CommandBuilder {
    /// Run a git command in the given directory.
    ///
    /// # Arguments
    /// * `cwd` - Working directory for the command
    /// * `args` - Git arguments (e.g., ["status", "--porcelain"])
    ///
    /// # Returns
    /// * `Ok(Output)` with stdout/stderr captured
    /// * `Err` if git command fails
    ///
    /// # Example
    /// ```ignore
    /// let output = CommandBuilder::git_command(&repo_path, &["status", "--porcelain"])?;
    /// let stdout = String::from_utf8(output.stdout)?;
    /// ```
    pub fn git_command<P: AsRef<Path>>(cwd: P, args: &[&str]) -> io::Result<Output> {
        let cwd = cwd.as_ref();
        let mut cmd = Command::new("git");
        cmd.current_dir(cwd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd.output()
    }

    /// Run a `git` subcommand with inherited stdio so it streams straight to the
    /// user's terminal (e.g. an interactive `git diff` / `git status` / `git log`
    /// the user reads on screen). Unlike [`git_command`], output is not captured.
    /// Spawn errors and non-zero exits are ignored, matching the call sites.
    pub fn git_interactive<P: AsRef<Path>>(cwd: P, args: &[&str]) {
        let _ = Command::new("git")
            .args(args)
            .current_dir(cwd.as_ref())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();
    }

    /// Run fusermount/umount in multiple fallback variants to unmount a FUSE filesystem.
    ///
    /// Tries all common unmount methods in order until one succeeds. This is a best-effort
    /// operation — succeeds if any variant returns success.
    ///
    /// # Arguments
    /// * `mount_point` - Path to unmount
    ///
    /// # Returns
    /// * `Ok(())` if any unmount variant succeeded
    /// * `Err` if all variants failed
    pub fn unmount_archive<P: AsRef<Path>>(mount_point: P) -> io::Result<()> {
        let mount_point = mount_point.as_ref();
        let path_str = mount_point.to_string_lossy();

        let variants: &[(&str, &[&str])] = &[
            ("fusermount",  &["-u",  &path_str]),
            ("fusermount3", &["-u",  &path_str]),
            ("fusermount",  &["-uz", &path_str]),
            ("fusermount3", &["-uz", &path_str]),
            ("umount",      &[        &path_str]),
            ("umount",      &["-l",   &path_str]),
        ];

        for (tool, args) in variants {
            if let Ok(status) = Command::new(tool)
                .args(*args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                && status.success() {
                    return Ok(());
                }
        }

        Err(io::Error::other(
            format!("Failed to unmount {}", path_str),
        ))
    }

    /// Append the `wget`/`curl` arguments that download `url` to `output_path`,
    /// requesting a progress bar on stderr. Returns an error for tools other
    /// than `wget`/`curl`.
    fn apply_download_args(
        cmd: &mut Command,
        tool: &str,
        url: &str,
        output_path: &Path,
    ) -> Result<(), String> {
        match tool {
            "wget" => {
                cmd.arg("--progress=bar:force")
                    .arg("--output-document")
                    .arg(output_path)
                    .arg("--")
                    .arg(url);
            }
            "curl" => {
                cmd.args(["--location", "--fail", "--output"])
                    .arg(output_path)
                    .arg("--progress-bar")
                    .arg(url);
            }
            other => return Err(format!("unsupported download tool: {}", other)),
        }
        Ok(())
    }

    /// Download from `url` to `output_path` using `wget` or `curl`, streaming stderr for progress.
    ///
    /// `on_progress` receives a short snippet suitable for a status/footer line. Callbacks are
    /// throttled and coalesced so callers can forward them to a UI without flooding.
    pub fn download_with_progress<F>(tool: &str, url: &str, output_path: &Path, mut on_progress: F) -> Result<(), String>
    where
        F: FnMut(&str),
    {
        fn stderr_progress_tail(buf: &[u8]) -> Option<String> {
            let s = String::from_utf8_lossy(buf);
            let piece = s
                .rsplit('\r')
                .next()
                .unwrap_or("")
                .rsplit('\n')
                .next()
                .unwrap_or("")
                .trim();
            if piece.is_empty() {
                return None;
            }
            const MAX: usize = 160;
            let count = piece.chars().count();
            let out: String = if count <= MAX {
                piece.to_string()
            } else {
                piece.chars().skip(count.saturating_sub(MAX)).collect()
            };
            Some(out)
        }

        let mut cmd = Command::new(tool);
        Self::apply_download_args(&mut cmd, tool, url, output_path)?;

        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let mut stderr = child.stderr.take().ok_or_else(|| "no stderr pipe".to_string())?;

        let mut raw_buf = Vec::new();
        let mut err_accum = String::new();
        let mut read_buf = [0u8; 4096];
        let mut last_emit = Instant::now().checked_sub(Duration::from_secs(1)).unwrap_or_else(Instant::now);
        let throttle = Duration::from_millis(200);
        let mut last_hint: Option<String> = None;

        loop {
            match stderr.read(&mut read_buf) {
                Ok(0) => break,
                Ok(n) => {
                    raw_buf.extend_from_slice(&read_buf[..n]);
                    if raw_buf.len() > 16 * 1024 {
                        let drain = raw_buf.len().saturating_sub(16 * 1024);
                        raw_buf.drain(..drain);
                    }
                    let chunk = String::from_utf8_lossy(&read_buf[..n]);
                    err_accum.push_str(&chunk);
                    if err_accum.len() > 48 * 1024 {
                        err_accum.drain(..err_accum.len().saturating_sub(48 * 1024));
                    }

                    if let Some(s) = stderr_progress_tail(&raw_buf) {
                        let elapsed_ok = last_emit.elapsed() >= throttle;
                        let changed = last_hint.as_deref() != Some(s.as_str());
                        if elapsed_ok || changed {
                            on_progress(&s);
                            last_emit = Instant::now();
                            last_hint = Some(s);
                        }
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            let tail = err_accum.trim();
            let detail = if tail.is_empty() {
                format!("{} exited with status {}", tool, status)
            } else {
                let t = tail
                    .rsplit('\r')
                    .next()
                    .unwrap_or("")
                    .rsplit('\n')
                    .next()
                    .unwrap_or("")
                    .trim();
                let tc = t.chars().count();
                let t = if tc > 512 {
                    t.chars().skip(tc.saturating_sub(512)).collect::<String>()
                } else {
                    t.to_string()
                };
                format!("{}: {}", tool, t)
            };
            Err(detail)
        }
    }
}
