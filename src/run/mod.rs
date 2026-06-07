//! Main TUI event loop (extracted from main.rs).

mod key_dispatch;
mod run_loop_body;

use std::{
    collections::{HashMap, HashSet},
    env,
    io::{self, Stdout},
    time::Duration,
};

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, KeyEvent},
    execute,
};
use ratatui::{prelude::*, widgets::*};
use unicode_width::UnicodeWidthStr;

use crate::{
    ui, App, AppMode, InternalSearchResult, InternalSearchScope, RemoteEntry,
};

pub(crate) fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    run_loop_body::run_tui_body(terminal, app)
}
