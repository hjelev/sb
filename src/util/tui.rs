use crossterm::{
    cursor::MoveTo,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use std::io;

/// Leave the TUI (enter normal terminal mode) before running an external process.
pub fn suspend_tui() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}

/// Re-enter the TUI after an external process has finished.
pub fn resume_tui() -> io::Result<()> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;
    Ok(())
}

/// Re-enter the alternate screen and clear it, e.g. after a full-screen external
/// program (pager, editor) has scribbled over the terminal.
///
/// Centralizes the repeated pair:
/// `execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;`
/// `execute!(stdout, TermClear(ClearType::All), MoveTo(0, 0))?;`
///
/// Note: this does not toggle raw mode; callers that need it continue to call
/// `enable_raw_mode()` themselves (the surrounding sites vary in ordering).
pub fn resume_tui_cleared() -> io::Result<()> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
    Ok(())
}
