//! Synchronous `sb.confirm`/`sb.input`/`sb.confirm_shell` primitives: suspend
//! the TUI, prompt on plain stdio (or run a live shell command), resume.
//! Only callable from `entry()` dispatch, where the caller holds the real
//! terminal handle (see `run_plugin_entry`).

use std::io::{self, Write};
use std::process::Command;

use crossterm::{
    cursor::{Hide, MoveLeft, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{Clear as TermClear, ClearType, disable_raw_mode, enable_raw_mode},
};

use crate::util::tui::{resume_tui, suspend_tui};

fn byte_index_for_char(s: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    s.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| s.len())
}

/// Redraw `label` + `buffer` on the first screen line, positioning the real
/// terminal cursor at the given char offset into `buffer`.
fn redraw_line(label: &str, buffer: &str, cursor: usize) -> io::Result<()> {
    let mut out = io::stdout();
    execute!(out, MoveTo(0, 0), TermClear(ClearType::CurrentLine))?;
    write!(out, "{}{}", label, buffer)?;
    let trailing = buffer.chars().count().saturating_sub(cursor);
    if trailing > 0 {
        execute!(out, MoveLeft(trailing as u16))?;
    }
    out.flush()
}

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

/// Prompt for a line of text with `prefill` pre-loaded into a real editable
/// buffer (arrow keys, Home/End, Backspace/Delete) — not just shown as a
/// hint — mirroring the built-in `AppMode::GitCommitMessage`/`GitTagInput`
/// overlays' editing feel. Enter submits the (possibly empty) buffer; Esc
/// cancels.
pub(crate) fn input(prompt: &str, prefill: Option<&str>) -> io::Result<Option<String>> {
    suspend_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0), Show)?;
    enable_raw_mode()?;

    let label = format!("{}: ", prompt);
    let mut buffer = prefill.unwrap_or("").to_string();
    let mut cursor = buffer.chars().count();

    let result = loop {
        redraw_line(&label, &buffer, cursor)?;
        let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? else {
            continue;
        };
        match code {
            KeyCode::Enter => break Some(buffer),
            KeyCode::Esc => break None,
            KeyCode::Left => cursor = cursor.saturating_sub(1),
            KeyCode::Right => cursor = (cursor + 1).min(buffer.chars().count()),
            KeyCode::Home => cursor = 0,
            KeyCode::End => cursor = buffer.chars().count(),
            KeyCode::Backspace if cursor > 0 => {
                let start = byte_index_for_char(&buffer, cursor - 1);
                let end = byte_index_for_char(&buffer, cursor);
                buffer.drain(start..end);
                cursor -= 1;
            }
            KeyCode::Delete if cursor < buffer.chars().count() => {
                let start = byte_index_for_char(&buffer, cursor);
                let end = byte_index_for_char(&buffer, cursor + 1);
                buffer.drain(start..end);
            }
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                buffer.clear();
                cursor = 0;
            }
            KeyCode::Char(c)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                let at = byte_index_for_char(&buffer, cursor);
                buffer.insert(at, c);
                cursor += 1;
            }
            _ => {}
        }
    };

    disable_raw_mode()?;
    execute!(io::stdout(), Hide)?;
    resume_tui()?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    Ok(result)
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
