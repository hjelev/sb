use std::{
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType,
    },
};

use crate::{App, AppMode, GitInfoCache};
use crate::util::background::spawn_worker;
use crate::util::command::CommandBuilder;
use crate::util::tui::{resume_tui, resume_tui_cleared, suspend_tui};

impl App {
    pub(crate) fn pump_git_info(&mut self) {
        let Some(rx) = self.git_info_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok((path, info)) => {
                self.git_info_cache = Some(GitInfoCache { path, info });
                self.git_info_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.git_info_rx = None;
            }
        }
    }

    pub(crate) fn request_git_info_for_current_dir_once(&mut self) {
        if !self.integration_enabled("git") {
            self.git_info_rx = None;
            self.git_info_cache = None;
            return;
        }
        if self.git_info_rx.is_some() {
            return;
        }
        let cache_is_current = self
            .git_info_cache
            .as_ref()
            .map(|cache| cache.path == self.current_dir)
            .unwrap_or(false);

        let is_fresh = self
            .git_last_check_at
            .map(|t| t.elapsed() < Duration::from_secs(8))
            .unwrap_or(false);

        if cache_is_current && is_fresh {
            return;
        }

        if !cache_is_current {
            // Clear stale data from a previously visited path until the new result arrives.
            self.git_info_cache = None;
        }

        let path = self.current_dir.clone();
        self.git_last_check_at = Some(Instant::now());
        self.git_info_rx = Some(spawn_worker(move |tx| {
            let info = App::get_git_info(&path);
            let _ = tx.send((path, info));
        }));
    }

    pub(crate) fn cached_git_info_for_current_dir(&self) -> Option<(&str, bool, Option<(&str, u64)>)> {
        let cache = self.git_info_cache.as_ref()?;
        if cache.path != self.current_dir {
            return None;
        }
        cache.info.as_ref().map(|(branch, dirty, tag)| {
            let tag_info = tag.as_ref().map(|(name, ahead)| (name.as_str(), *ahead));
            (branch.as_str(), *dirty, tag_info)
        })
    }

    pub(crate) fn get_git_info(path: &PathBuf) -> Option<(String, bool, Option<(String, u64)>)> {
        let branch = CommandBuilder::git_command(path, &["symbolic-ref", "--short", "-q", "HEAD"])
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if value.is_empty() { None } else { Some(value) }
                } else {
                    None
                }
            })
            .or_else(|| {
                CommandBuilder::git_command(path, &["rev-parse", "--short", "HEAD"])
                    .ok()
                    .and_then(|out| {
                        if out.status.success() {
                            let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            if value.is_empty() { None } else { Some(value) }
                        } else {
                            None
                        }
                    })
            })?;

        // Fast tracked-change dirty check: exit code 1 means dirty, 0 means clean.
        let dirty_status = CommandBuilder::git_command(path, &["diff-index", "--quiet", "HEAD", "--"])
            .ok()?;

        let tracked_dirty = match dirty_status.status.code() {
            Some(0) => false,
            Some(1) => true,
            _ => return None,
        };

        let has_untracked = CommandBuilder::git_command(path, &["ls-files", "--others", "--exclude-standard"])
            .ok()
            .map(|out| !out.stdout.is_empty())
            .unwrap_or(false);

        let is_dirty = tracked_dirty || has_untracked;

        let latest_tag = CommandBuilder::git_command(
            path,
            &[
                "for-each-ref",
                "refs/tags",
                "--sort=-v:refname",
                "--count=1",
                "--format=%(refname:short)",
            ],
        )
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if value.is_empty() { None } else { Some(value) }
            } else {
                None
            }
        });

        let tag_info = latest_tag.map(|tag| {
            let ahead = CommandBuilder::git_command(path, &["rev-list", "--count", &format!("{}..HEAD", tag)])
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        String::from_utf8_lossy(&out.stdout)
                            .trim()
                            .parse::<u64>()
                            .ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            (tag, ahead)
        });

        Some((branch, is_dirty, tag_info))
    }

    pub(crate) fn parse_git_commit_message(raw: &str) -> (String, bool) {
        let mut amend = false;
        let mut parts: Vec<&str> = Vec::new();
        for token in raw.split_whitespace() {
            if token == "--amend" {
                amend = true;
            } else {
                parts.push(token);
            }
        }
        (parts.join(" "), amend)
    }

    pub(crate) fn latest_git_tag(&self) -> Option<String> {
        let out = CommandBuilder::git_command(&self.current_dir, &["describe", "--tags", "--abbrev=0"])
            .ok()?;

        if !out.status.success() {
            return None;
        }

        let tag = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if tag.is_empty() {
            None
        } else {
            Some(tag)
        }
    }

    pub(crate) fn preview_git_diff_and_confirm_commit(&mut self) -> io::Result<bool> {
        suspend_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;

        let delta_available = self.integration_active("delta");
        if delta_available {
            println!("$ git -c core.pager=delta -c delta.side-by-side=true -c delta.features=side-by-side diff");
            let _ = Command::new("git")
                .args([
                    "-c",
                    "core.pager=delta",
                    "-c",
                    "delta.side-by-side=true",
                    "-c",
                    "delta.features=side-by-side",
                    "diff",
                ])
                .current_dir(&self.current_dir)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
        } else {
            println!("$ git -c color.ui=always diff");
            let _ = Command::new("git")
                .args(["-c", "color.ui=always", "diff"])
                .current_dir(&self.current_dir)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
            println!("\nTip: install delta for side-by-side colored diff preview.");
        }

        println!("\n$ git status");
        let _ = Command::new("git")
            .arg("status")
            .current_dir(&self.current_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        print!("\nDo you really want to commit these changes? [y/N]: ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        let _ = io::stdin().read_line(&mut answer);
        let confirmed = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");

        resume_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;

        Ok(confirmed)
    }

    pub(crate) fn run_git_commit_and_push(&mut self, commit_message: &str, amend: bool) -> io::Result<()> {
        suspend_tui()?;

        let mut failed_step: Option<String> = None;
        let mut push_forced = false;
        let run_step = |args: &[&str], dir: &PathBuf| -> io::Result<bool> {
            let out = CommandBuilder::git_command(dir, args)?;
            Ok(out.status.success())
        };

        println!("$ git add --all");
        if !run_step(&["add", "--all"], &self.current_dir)? {
            failed_step = Some("git add --all failed".to_string());
        }

        if failed_step.is_none() {
            if amend {
                println!("$ git commit -m \"{}\" --amend", commit_message);
                if !run_step(&["commit", "-m", commit_message, "--amend"], &self.current_dir)? {
                    failed_step = Some("git commit --amend failed".to_string());
                }
            } else {
                println!("$ git commit -m \"{}\"", commit_message);
                if !run_step(&["commit", "-m", commit_message], &self.current_dir)? {
                    failed_step = Some("git commit failed".to_string());
                }
            }
        }

        if failed_step.is_none() && !amend {
            println!("$ git pull --rebase");
            if !run_step(&["pull", "--rebase"], &self.current_dir)? {
                failed_step = Some("git pull --rebase failed (resolve conflicts manually)".to_string());
            }
        }

        if failed_step.is_none() {
            if amend {
                println!("$ git push origin HEAD -f");
                push_forced = true;
                if !run_step(&["push", "origin", "HEAD", "-f"], &self.current_dir)? {
                    failed_step = Some("git push -f failed".to_string());
                }
            } else {
                println!("$ git push origin HEAD");
                if !run_step(&["push", "origin", "HEAD"], &self.current_dir)? {
                    failed_step = Some("git push failed".to_string());
                }
            }
        }

        let mut tag_requested = false;
        if failed_step.is_none() {
            println!("\nPress any key to return to shell buddy, or press 't' to create+push a tag...");
            let _ = io::stdout().flush();
            enable_raw_mode()?;
            loop {
                if let Event::Key(key) = event::read()? {
                    tag_requested = matches!(key.code, KeyCode::Char('t') | KeyCode::Char('T'));
                    break;
                }
            }
            disable_raw_mode()?;
        } else {
            println!("\nPress any key to return to shell buddy...");
            let _ = io::stdout().flush();
            enable_raw_mode()?;
            loop {
                if let Event::Key(_) = event::read()? {
                    break;
                }
            }
            disable_raw_mode()?;
        }

        resume_tui_cleared()?;
        enable_raw_mode()?;

        if let Some(step) = failed_step {
            self.set_status(step);
        } else if push_forced {
            self.set_status("amend commit pushed with -f");
            if tag_requested {
                let prefill = self.latest_git_tag().unwrap_or_else(|| "v0.1.0".to_string());
                self.begin_input_edit(AppMode::GitTagInput, prefill);
                self.set_status("edit tag and press Enter to create+push (Esc=cancel)");
            }
        } else {
            self.set_status("commit pushed");
            if tag_requested {
                let prefill = self.latest_git_tag().unwrap_or_else(|| "v0.1.0".to_string());
                self.begin_input_edit(AppMode::GitTagInput, prefill);
                self.set_status("edit tag and press Enter to create+push (Esc=cancel)");
            }
        }

        self.refresh_entries_or_status();
        self.git_info_cache = None;
        self.request_git_info_for_current_dir_once();
        Ok(())
    }

    pub(crate) fn run_git_tag_and_push(&mut self, tag: &str) -> io::Result<()> {
        suspend_tui()?;

        let run_step = |args: &[&str], dir: &PathBuf| -> io::Result<bool> {
            let out = CommandBuilder::git_command(dir, args)?;
            Ok(out.status.success())
        };

        let mut failed_step: Option<String> = None;

        println!("$ git tag {}", tag);
        if !run_step(&["tag", tag], &self.current_dir)? {
            failed_step = Some("git tag failed".to_string());
        }

        if failed_step.is_none() {
            println!("$ git push origin {}", tag);
            if !run_step(&["push", "origin", tag], &self.current_dir)? {
                failed_step = Some("git push tag failed".to_string());
            }
        }

        println!("\nPress Enter to return to shell buddy...");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);

        resume_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;

        if let Some(step) = failed_step {
            self.set_status(step);
        } else {
            self.set_status(format!("tag pushed: {}", tag));
        }

        self.refresh_entries_or_status();
        self.git_info_cache = None;
        self.request_git_info_for_current_dir_once();
        Ok(())
    }
}
