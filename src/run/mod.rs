//! Main TUI event loop (extracted from main.rs).

mod key_dispatch;
mod render_footer;
mod render_header;
mod render_overlays;
mod render_table;
mod render_types;
mod render_util;
mod run_loop_body;

use render_footer::*;
use render_header::*;
use render_overlays::*;
use render_table::*;
use render_types::*;
use render_util::*;

use std::{
    collections::HashSet,
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
