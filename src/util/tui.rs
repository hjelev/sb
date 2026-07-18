use crossterm::{
    cursor::{Hide, MoveTo, Show},
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

/// Which resume routine a [`TuiGuard`] runs when it is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeMode {
    /// [`resume_tui`] — re-enter the alt screen + mouse capture + raw mode.
    Plain,
    /// [`resume_tui_cleared`] followed by `enable_raw_mode()` — additionally
    /// clears the screen, for use after a full-screen external program.
    Cleared,
}

/// RAII guard returned by [`suspend`] / [`suspend_showing_cursor`].
///
/// Suspends the TUI when created and runs the matching resume **exactly once**
/// when dropped — on normal fall-through, on an early `?`/`return`, and during
/// panic unwinding. This centralizes the `suspend_tui()? … resume_tui()?` pairs
/// that were duplicated across ~20 call sites and closes the bug where an error
/// between the two left the terminal stuck in raw mode / the alternate screen.
///
/// Scope the guard to exactly the region that must run outside the TUI; anything
/// that must happen *after* the terminal is restored (e.g. `terminal.clear()`)
/// belongs after the guard's scope:
/// ```ignore
/// {
///     let _tui = tui::suspend(ResumeMode::Plain)?;
///     let _ = Command::new("less").arg(path).status();
/// } // <- resume happens here
/// terminal.clear()?;
/// ```
#[must_use = "bind the guard to a named variable; `let _ = suspend(..)` drops it immediately and resumes right away"]
pub struct TuiGuard {
    mode: ResumeMode,
    cursor: bool,
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = match self.mode {
            ResumeMode::Plain => resume_tui(),
            ResumeMode::Cleared => resume_tui_cleared().and_then(|()| enable_raw_mode()),
        };
        if self.cursor {
            let _ = execute!(io::stdout(), Hide);
        }
    }
}

/// Suspend the TUI, returning a [`TuiGuard`] that resumes with `mode` on drop.
pub fn suspend(mode: ResumeMode) -> io::Result<TuiGuard> {
    suspend_tui()?;
    Ok(TuiGuard { mode, cursor: false })
}

/// Like [`suspend`], but shows the cursor while the TUI is suspended (for
/// interactive editors/shells) and hides it again when the guard resumes.
pub fn suspend_showing_cursor(mode: ResumeMode) -> io::Result<TuiGuard> {
    suspend_tui()?;
    execute!(io::stdout(), Show)?;
    Ok(TuiGuard { mode, cursor: true })
}
