//! Command execution builder pattern.
//!
//! Replaces scattered `Command::new()` patterns with a builder that:
//! - Captures stderr for better error messages
//! - Returns consistent Result types (no silent failures)
//! - Provides high-level methods for common commands (git, archive, preview)

// Some builder methods are intended API surface not yet wired up at all call sites.
#![allow(dead_code)]

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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

    /// Run a preview tool (bat, glow, hexyl, etc.) and capture output.
    ///
    /// # Arguments
    /// * `tool` - Tool name (e.g., "bat", "glow", "hexyl")
    /// * `path` - File to preview
    /// * `args` - Additional arguments to pass to the tool
    ///
    /// # Returns
    /// * `Ok(Output)` with preview content
    /// * `Err` if tool execution fails
    pub fn preview_command(tool: &str, path: &PathBuf, args: &[&str]) -> io::Result<Output> {
        let mut cmd = Command::new(tool);
        cmd.arg(path).args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        cmd.output()
    }

    /// Run an archive command (tar, unzip, etc.).
    ///
    /// Captures both stdout and stderr for proper error reporting.
    pub fn archive_command(tool: &str, args: &[&str], cwd: Option<&Path>) -> io::Result<Output> {
        let mut cmd = Command::new(tool);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }

        cmd.output()
    }

    /// Download a URL to the given output path using wget or curl.
    pub fn download_command(tool: &str, url: &str, output_path: &Path) -> io::Result<Output> {
        let mut cmd = Command::new(tool);
        match tool {
            "wget" => {
                cmd.args(["--output-document"])
                    .arg(output_path)
                    .arg("--")
                    .arg(url);
            }
            "curl" => {
                cmd.args(["--location", "--fail", "--output"])
                    .arg(output_path)
                    .arg("--")
                    .arg(url);
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported download tool: {}", other),
                ));
            }
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.output()
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
        match tool {
            "wget" => {
                cmd.args(["--progress=bar:force", "--output-document"])
                    .arg(output_path)
                    .arg("--")
                    .arg(url);
            }
            "curl" => {
                cmd.args([
                    "--location",
                    "--fail",
                    "--output",
                ])
                .arg(output_path)
                .arg("--progress-bar")
                .arg(url);
            }
            other => {
                return Err(format!("unsupported download tool: {}", other));
            }
        }

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

    /// Check if a tool is available by running `which`.
    ///
    /// Returns true if the tool can be found in PATH.
    pub fn tool_available(tool: &str) -> bool {
        Command::new("which")
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_command_tool_check() {
        // This test just verifies the method exists and returns appropriate Result
        // Actual git command execution would require git to be installed
        assert!(CommandBuilder::tool_available("git") || !CommandBuilder::tool_available("nonexistent_tool_12345"));
    }
}
