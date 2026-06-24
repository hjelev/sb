use std::{
    env, fs,
    io::{self, Stdout},
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType},
};
use ratatui::prelude::*;

use crate::{App, AppMode, DualPanelSide, InternalSearchScope, RemoteEntry};

mod browsing;
mod internal_search;
mod key_dispatch_body;
mod ssh_picker;

use browsing::*;
use internal_search::*;
use ssh_picker::*;

pub(crate) enum KeyDispatchOutcome {
    Ok,
    Quit,
    ContinueLoop,
}

pub(crate) fn handle_app_key_event(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
    deferred_key: &mut Option<KeyEvent>,
) -> io::Result<KeyDispatchOutcome> {
    key_dispatch_body::handle_app_key_event_body(terminal, app, key, deferred_key)
}
