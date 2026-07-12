//! Synchronous `sb.confirm`/`sb.input`/`sb.confirm_shell` primitives: suspend
//! the TUI, prompt on plain stdio (or run a live shell command), resume.
//! Only callable from `entry()` dispatch, where the caller holds the real
//! terminal handle (see `run_plugin_entry`).

use std::io::{self, Write};
use std::process::Command;

use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear as TermClear, ClearType},
};

use crate::util::tui::{resume_tui, suspend_tui};

pub(crate) fn confirm(prompt: &str) -> io::Result<bool> {
    suspend_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    print!("{}\n[y/N]: ", prompt);
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let confirmed = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    resume_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    Ok(confirmed)
}

pub(crate) fn input(prompt: &str, prefill: Option<&str>) -> io::Result<Option<String>> {
    suspend_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    match prefill {
        Some(p) if !p.is_empty() => print!("{} [{}]: ", prompt, p),
        _ => print!("{}: ", prompt),
    }
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    resume_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    let trimmed = line.trim();
    Ok(if trimmed.is_empty() {
        prefill.map(str::to_string)
    } else {
        Some(trimmed.to_string())
    })
}

/// Run `cmd` with the terminal's real, inherited stdio (so pagers/colorizers
/// like `delta` render exactly as they would from a plain shell), then ask a
/// yes/no question on the same still-suspended screen and return the answer.
pub(crate) fn confirm_shell(cmd: &str, question: &str) -> io::Result<bool> {
    suspend_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    println!("$ {}", cmd);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let _ = Command::new(&shell).args(["-c", cmd]).status();
    print!("\n{}\n[y/N]: ", question);
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let confirmed = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    resume_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    Ok(confirmed)
}
