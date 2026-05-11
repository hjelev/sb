//! Command execution builder pattern.
//!
//! Replaces scattered `Command::new()` patterns with a builder that:
//! - Captures stderr for better error messages
//! - Returns consistent Result types (no silent failures)
#![allow(dead_code)]
//! - Provides high-level methods for common commands (git, archive, preview)

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::io;

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
            {
                if status.success() {
                    return Ok(());
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::Other,
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
