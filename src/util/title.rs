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
    path::Path,
    sync::Mutex,
};

const DEFAULT_ICON: &str = "📂";

/// Last title we emitted; `None` forces the next `update` to re-send it.
static LAST: Mutex<Option<String>> = Mutex::new(None);

/// `SB_TERM_TITLE=0` leaves the terminal title untouched.
fn enabled() -> bool {
    env::var("SB_TERM_TITLE").map(|v| v != "0").unwrap_or(true)
}

fn icon() -> String {
    env::var("SB_TITLE_ICON").unwrap_or_else(|_| DEFAULT_ICON.to_string())
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
    if enabled() {
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
    let title = format_title(&icon(), dir);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.as_deref() == Some(title.as_str()) {
        return;
    }
    if execute!(io::stdout(), SetTitle(&title)).is_ok() {
        *last = Some(title);
    }
}

/// External programs (editors, pagers) may rewrite the title while sb is
/// suspended; forget what we sent so the next `update` re-applies it.
pub fn invalidate() {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Restore the title saved by `save`.
pub fn restore() {
    if enabled() {
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
