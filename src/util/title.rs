//! Terminal window/tab title (OSC 0) with a leading icon glyph.
//!
//! Terminals can't take a real image for the tab icon, so — like other TUIs —
//! we prefix the title with a glyph that tab bars render as the "icon".
//! The previous title is saved on the xterm title stack on start and restored
//! on exit (terminals without the stack simply ignore those sequences).

use crossterm::{execute, terminal::SetTitle};
use std::{
    env,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

const DEFAULT_ICON: &str = "📂";

/// Directory of the last title we emitted; `None` forces the next `update`
/// to re-send it. Compared before formatting so the per-tick call is cheap.
static LAST: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Whether `save` pushed a title that `restore` still has to pop.
static SAVED: AtomicBool = AtomicBool::new(false);

/// `SB_TERM_TITLE=0` leaves the terminal title untouched. Read once.
fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| env::var("SB_TERM_TITLE").map(|v| v != "0").unwrap_or(true))
}

/// Read once; `update` runs on every loop tick.
fn icon() -> &'static str {
    static ICON: OnceLock<String> = OnceLock::new();
    ICON.get_or_init(|| env::var("SB_TITLE_ICON").unwrap_or_else(|_| DEFAULT_ICON.to_string()))
}

fn format_title(icon: &str, dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.to_string_lossy().into_owned());
    if icon.is_empty() {
        format!("sb: {name}")
    } else {
        format!("{icon} sb: {name}")
    }
}

/// Save the current terminal title so `restore` can put it back.
pub fn save() {
    if enabled() && !SAVED.swap(true, Ordering::SeqCst) {
        let mut out = io::stdout();
        let _ = out.write_all(b"\x1b[22;0t");
        let _ = out.flush();
    }
}

/// Set the title for `dir`, skipping the write when nothing changed.
pub fn update(dir: &Path) {
    if !enabled() {
        return;
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.as_deref() == Some(dir) {
        return;
    }
    let title = format_title(icon(), dir);
    if execute!(io::stdout(), SetTitle(&title)).is_ok() {
        *last = Some(dir.to_path_buf());
    }
}

/// External programs (editors, pagers) may rewrite the title while sb is
/// suspended; forget what we sent so the next `update` re-applies it.
pub fn invalidate() {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Restore the title saved by `save`.
pub fn restore() {
    // Pop only what `save` pushed, once: popping more would discard a title
    // the shell or tmux had saved.
    if enabled() && SAVED.swap(false, Ordering::SeqCst) {
        let mut out = io::stdout();
        let _ = out.write_all(b"\x1b[23;0t");
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_uses_dir_name_and_icon() {
        assert_eq!(format_title("📂", Path::new("/home/u/src")), "📂 sb: src");
        assert_eq!(format_title("", Path::new("/home/u/src")), "sb: src");
        assert_eq!(format_title("📂", Path::new("/")), "📂 sb: /");
    }
}
