//! Yazi-style Lua plugin system.
//!
//! Plugins live in `~/.config/sb/plugins/` as either `<name>/main.lua` or a
//! flat `<name>.lua` (the directory form shadows the flat form). Each script
//! must return a module table; the fields sbrs inspects are:
//!
//! - `entry(ctx)` — a key-bindable command (bound via `plugin_key_<name>`
//!   config lines or run with `;` + `:<name>`).
//! - `peek(ctx)` + `preview = { exts = {...} }` — a custom previewer for the
//!   listed extensions (`"*"` matches any file, tried after exact matches).
//! - `setup()` — optional one-time init at load (main thread only; not
//!   re-run in preview workers).
//! - `on_start(ctx)` / `on_cd(ctx)` / `on_select(ctx)` / `on_quit(ctx)` —
//!   event hooks.
//!
//! Calls into Lua never borrow `App`: each call receives a read-only context
//! snapshot plus an `sb` API table whose functions queue [`PluginEffect`]s,
//! applied to `&mut App` after the call returns. Lua errors are contained
//! (recorded on the plugin, surfaced in the status line / Plugins panel) and
//! must never crash the TUI.

pub(crate) mod api;
pub(crate) mod discovery;
pub(crate) mod effects;
pub(crate) mod preview;
pub(crate) mod runtime;

use std::path::PathBuf;

use crate::util::keymap::KeyCombo;

/// A plugin script discovered on disk, before loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginSource {
    /// Directory name or file stem; also the config/launcher identifier.
    pub name: String,
    /// Absolute path to the Lua script (`.../plugins/<name>/main.lua` or
    /// `.../plugins/<name>.lua`).
    pub script: PathBuf,
}

/// Which event hooks a plugin's module table defines.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PluginHooks {
    pub start: bool,
    pub cd: bool,
    pub select: bool,
    pub quit: bool,
}

/// Post-load metadata and state for one plugin.
pub(crate) struct LoadedPlugin {
    pub source: PluginSource,
    /// Mirrors the persisted `disabled_plugins` list (default: enabled).
    pub enabled: bool,
    pub has_entry: bool,
    /// Lowercased extensions from `M.preview.exts`; `"*"` = catch-all.
    pub preview_exts: Vec<String>,
    pub hooks: PluginHooks,
    /// Registry handle to the module table returned by `main.lua`.
    /// `None` when disabled or the load failed (see `last_error`).
    pub module_key: Option<mlua::RegistryKey>,
    pub last_error: Option<String>,
    /// Key bound to `entry()`, from `plugin_key_<name>` config lines.
    pub bound_key: Option<KeyCombo>,
}

/// Side-effects a plugin call may request via the `sb` API; drained and
/// applied to `&mut App` after the Lua call returns. `EditPath`/`ViewPath`/
/// `RunShellWait` suspend the TUI and are only honored where a terminal
/// handle is in scope (entry dispatch), never from hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginEffect {
    Cd(PathBuf),
    Status(String),
    RefreshDir,
    SelectName(String),
    MarkNames(Vec<String>),
    ClearMarks,
    ClipboardSet(Vec<PathBuf>),
    EditPath(PathBuf),
    ViewPath(PathBuf),
    RunShellWait(String),
}

impl PluginEffect {
    /// Effects that suspend the TUI and therefore need the terminal handle.
    pub fn needs_terminal(&self) -> bool {
        matches!(
            self,
            Self::EditPath(_) | Self::ViewPath(_) | Self::RunShellWait(_)
        )
    }
}

/// Messages sent back from plugin worker threads (`sb.spawn`), drained by
/// `App::pump_plugins()` in the run loop.
pub(crate) enum PluginMsg {
    SpawnDone {
        /// Callback token issued by the runtime when the spawn was queued.
        token: u64,
        plugin: String,
        status: i32,
        stdout: String,
        stderr: String,
    },
}

/// Previewer registration handed to the preview worker thread. Plain data
/// (`Send`) — the worker instantiates its own Lua to run `peek()`.
#[derive(Debug, Clone)]
pub(crate) struct PreviewerReg {
    pub plugin: String,
    pub script: PathBuf,
    pub exts: Vec<String>,
}

/// Read-only snapshot of app state passed into `entry()` and hooks.
#[derive(Debug, Clone, Default)]
pub(crate) struct PluginCtx {
    pub cwd: PathBuf,
    pub selected: Option<PathBuf>,
    /// Marked entry paths, or the selected path when nothing is marked.
    pub files: Vec<PathBuf>,
    /// `"left"` or `"right"`.
    pub panel: &'static str,
    /// Previous directory (for `on_cd`).
    pub prev_dir: Option<PathBuf>,
}
