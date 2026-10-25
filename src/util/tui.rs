use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        Clear as TermClear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
        disable_raw_mode, enable_raw_mode,
    },
};
use std::io;
use std::process::Child;
use std::time::Duration;

/// Leave the TUI (enter normal terminal mode) before running an external process.
pub fn suspend_tui() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}

/// Re-enter the TUI after an external process has finished.
pub fn resume_tui() -> io::Result<()> {
    super::title::invalidate();
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
    super::title::invalidate();
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
    Ok(TuiGuard {
        mode,
        cursor: false,
    })
}

/// Like [`suspend`], but shows the cursor while the TUI is suspended (for
/// interactive editors/shells) and hides it again when the guard resumes.
pub fn suspend_showing_cursor(mode: ResumeMode) -> io::Result<TuiGuard> {
    suspend_tui()?;
    execute!(io::stdout(), Show)?;
    Ok(TuiGuard { mode, cursor: true })
}

/// RAII guard for the whole TUI session, returned by [`enter_session`].
///
/// Restores the terminal (raw mode off, mouse capture off, main screen,
/// cursor shown, title restored) when dropped, so an early `?` anywhere
/// between setup and shutdown can't leave the user's shell unusable.
/// Call [`SessionGuard::leave`] on the normal exit path to see restore errors.
#[must_use = "dropping the guard immediately restores the terminal"]
pub struct SessionGuard {
    active: bool,
}

impl SessionGuard {
    /// Restore the terminal now, reporting any error.
    pub fn leave(mut self) -> io::Result<()> {
        self.active = false;
        leave_session()
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = leave_session();
        }
    }
}

/// Enter raw mode + the alternate screen with mouse capture, and save the
/// terminal title. The returned guard undoes all of it.
pub fn enter_session() -> io::Result<SessionGuard> {
    enable_raw_mode()?;
    // Construct the guard before the remaining steps so a failure below still
    // turns raw mode back off.
    let guard = SessionGuard { active: true };
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    super::title::save();
    Ok(guard)
}

/// Best-effort full terminal restore; every step runs even if one fails.
fn leave_session() -> io::Result<()> {
    let raw = disable_raw_mode();
    let screen = execute!(
        io::stdout(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    );
    super::title::restore();
    raw.and(screen)
}

/// Install a panic hook that restores the terminal before the panic message is
/// printed, then unmounts any FUSE mounts we own.
///
/// The release profile uses `panic = "abort"`, so no destructors (including
/// [`SessionGuard`]) run on panic — in any thread. Without this hook a panic
/// leaves the shell in raw mode on the alternate screen, with the message
/// printed where nobody can see it, and archive/remote mounts left behind.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = leave_session();
        default_hook(info);
        super::cleanup::unmount_registered_on_crash();
    }));
}

/// Enables raw mode for its lifetime; turns it off again when dropped, even
/// on an early `?` return.
#[must_use = "dropping the guard immediately disables raw mode"]
pub struct RawModeGuard(());

impl RawModeGuard {
    pub fn enable() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self(()))
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

/// Wait for `child` (e.g. an audio player) to exit, stopping it early on
/// q / Esc / Left. Raw mode is on only while waiting. If polling fails the
/// child is killed rather than left playing in the background.
pub fn wait_child_or_stop_key(child: &mut Child) -> io::Result<()> {
    use crossterm::event::{self, Event, KeyCode};

    let _raw = RawModeGuard::enable()?;
    let result = (|| -> io::Result<()> {
        loop {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            if event::poll(Duration::from_millis(120))?
                && let Event::Key(k) = event::read()?
                && matches!(k.code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
