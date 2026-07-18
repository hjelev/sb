impl App {
    fn mode_shows_main_scrollbar(&self) -> bool {
        matches!(self.mode, AppMode::Browsing | AppMode::PathEditing)
    }

    fn is_preview_mode(&self) -> bool {
        self.view_mode == ViewMode::Preview
    }

    fn is_dual_panel_mode(&self) -> bool {
        self.view_mode == ViewMode::DualPanel
    }
}

const MAIN_LIST_DOUBLE_CLICK_WINDOW_MS: u64 = 320;

use crossterm::{
    cursor::MoveTo,
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use chrono::Local;
use regex::Regex;
use ratatui::{prelude::*, widgets::*};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    fs,
    io,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod integration;
mod app_ai;
mod app_archive;
mod app_core;
mod app_download;
mod app_entry_iter;
mod app_git;
mod app_images;
mod app_init;
mod app_input;
mod app_files;
mod app_meta;
mod app_mouse;
mod app_notes;
mod app_organize;
mod app_plugins;
mod app_preview;
mod app_remote;
mod app_rendering;
mod app_render_cache;
pub(crate) use app_render_cache::{EntryRenderCache, EntryRenderConfig, ListAggregates};
mod app_model;
pub(crate) use app_model::*;
mod app_search;
mod app_sizes;
mod app_sort;
mod app_shell;
mod app_shortcuts;
mod app_sqlite;
mod app_transfer;
mod plugin;
mod ui;
mod util;
mod run;

use integration::rows::IntegrationRow;
use util::background::AsyncJobState;

struct PanelState {
    dir: PathBuf,
    entries: Vec<fs::DirEntry>,
    entry_render_cache: Vec<EntryRenderCache>,
    list_aggregates: ListAggregates,
    selected_index: usize,
    marked_indices: HashSet<usize>,
    table_state: TableState,
    sort_mode: SortMode,
    show_hidden: bool,
    folder_filter: Option<PathInputFilter>,
    list_scroll_dragging: bool,
    list_scroll_grab_offset: u16,
    list_last_click: Option<(PathBuf, usize, Instant)>,
    tree_row_prefixes: Vec<String>,
    selected_total_size: AsyncJobState<SelectedTotalSizeMsg>,
    selected_total_size_pending: bool,
    selected_total_size_bytes: Option<u64>,
    selected_total_size_items: usize,
}

/// Progress and bookkeeping for an in-flight copy/move transfer.
///
/// Grouped out of `App` so all the parallel `copy_*` fields live together and
/// can be reset as a unit.
#[derive(Default)]
struct CopyOperation {
    rx: Option<Receiver<CopyProgressMsg>>,
    total_rx: Option<Receiver<u64>>,
    total_bytes: u64,
    done_bytes: u64,
    job_total_bytes: u64,
    done_before_job: u64,
    started_at: Option<Instant>,
    item_name: String,
    current_src: Option<PathBuf>,
    from_remote: bool,
}

/// Paste/move queue state plus in-flight download bookkeeping. Grouped out of
/// `App` so the parallel `paste_*` / `download_*` fields live together and can
/// be reset as a unit.
#[derive(Default)]
struct TransferState {
    paste_queue: VecDeque<PathBuf>,
    paste_current_src: Option<PathBuf>,
    paste_move_mode: bool,
    paste_target_dir: Option<PathBuf>,
    paste_total_items: usize,
    paste_ok_items: usize,
    paste_failed_items: usize,
    download_rx: Option<Receiver<DownloadProgressMsg>>,
    download_pending_url: Option<String>,
    download_pending_name: Option<String>,
    download_resume_input: Option<String>,
    download_active_name: String,
}

/// Bookmarks overlay UI state: the selected slot, in-progress edit/delete slot
/// indices, and the cached bookmark slots (see [`App::load_bookmarks`]) so the
/// render path never touches the config file per frame.
#[derive(Default)]
struct BookmarkUiState {
    selected: usize,
    edit_idx: usize,
    delete_idx: usize,
    cache: Vec<(usize, Option<PathBuf>)>,
}

/// Help overlay scroll position plus the native (kitty/sixel) logo-image render
/// bookkeeping. Grouped out of `App`.
#[derive(Default)]
struct HelpState {
    scroll_offset: u16,
    max_offset: u16,
    logo_native_area: Option<Rect>,
    logo_native_last_key: Option<String>,
    logo_native_last_area: Option<Rect>,
}

/// Background git-info cache and the in-flight probe channel + last-check
/// timestamp. Grouped out of `App`.
#[derive(Default)]
struct GitInfoState {
    info_cache: Option<GitInfoCache>,
    info_rx: Option<Receiver<(PathBuf, Option<GitInfo>)>>,
    last_check_at: Option<Instant>,
}

/// AI organize-plan review state: the request channel, the proposed plan and its
/// target directory, and the overlay's scroll/button focus. Grouped out of `App`.
#[derive(Default)]
struct OrganizeState {
    rx: Option<Receiver<OrganizePlanMsg>>,
    plan: Option<OrganizePlan>,
    work_dir: Option<PathBuf>,
    scroll_offset: u16,
    max_offset: u16,
    button_focus: u8,
}

/// SQLite preview overlay state: the db path, table list + selection, the
/// rendered output lines, the per-table row limit, and any error. Grouped out
/// of `App`.
#[derive(Default)]
struct DbPreviewState {
    path: Option<PathBuf>,
    tables: Vec<String>,
    selected: usize,
    output_lines: Vec<String>,
    row_limit: usize,
    error: Option<String>,
}

/// Integrations panel state: selected row, per-tool enable/disable overrides,
/// the cached row list, the search box, and the pending brew-install prompt.
/// Grouped out of `App`.
#[derive(Default)]
struct IntegrationUiState {
    selected: usize,
    overrides: HashMap<String, bool>,
    rows_cache: Vec<IntegrationRow>,
    search_active: bool,
    search_query: String,
    install_key: Option<String>,
    install_package: Option<String>,
    install_brew_path: Option<String>,
}

/// Delete-confirmation overlay state: the captured targets, scroll position, and
/// button focus (plus the bookmark-delete confirmation variant). Grouped out of
/// `App`.
#[derive(Default)]
struct ConfirmDeleteState {
    targets: Vec<ui::dialogs::DeleteTarget>,
    scroll_offset: u16,
    max_offset: u16,
    button_focus: u8,
    bookmark_button_focus: u8,
}

/// Themes panel selection state: the selected theme row plus the focus flags for
/// the Nerd-Fonts / filename-colors / disable-clock toggle rows. Grouped out of
/// `App`.
#[derive(Default)]
struct ThemePanelState {
    selected: usize,
    nerd_selected: bool,
    color_selected: bool,
    clock_selected: bool,
}

/// AI commit-message settings + runtime state: provider/model/key edit buffers,
/// stored per-provider keys, the auto-commit flag, the in-flight commit-request
/// and key-validation channels, and key-validation bookkeeping. Grouped out of
/// `App`.
#[derive(Default)]
struct AiState {
    provider: String,
    model: String,
    api_key: String,
    api_keys: HashMap<String, String>,
    auto_commit: bool,
    commit_rx: Option<Receiver<AiCommitMsg>>,
    key_status: AiKeyStatus,
    key_checked: Option<String>,
    key_edit_at: Option<Instant>,
    key_check_rx: Option<Receiver<AiKeyCheckMsg>>,
}

/// Per-file notes state for the left (and, in dual-panel mode, right) directory:
/// loaded name→note maps, async load channels + scan id, and the in-progress
/// note-edit targets/dir. Grouped out of `App`.
#[derive(Default)]
struct NotesState {
    by_name: HashMap<String, String>,
    rx: Option<Receiver<NotesLoadMsg>>,
    scan_id: u64,
    loaded_for: Option<PathBuf>,
    right_by_name: HashMap<String, String>,
    right_rx: Option<Receiver<NotesLoadMsg>>,
    right_loaded_for: Option<PathBuf>,
    edit_targets: Vec<String>,
    edit_dir: PathBuf,
}

/// Preview pane state: the loaded content lines + kinds, scroll offset, footer,
/// the async content-load channel/request id + pending flag, the rendered-content
/// cache, native/bitmap image render state, and which pane has focus. Grouped
/// out of `App`.
#[derive(Default)]
struct PreviewState {
    scroll_offset: usize,
    target_path: Option<PathBuf>,
    lines: Vec<String>,
    line_kinds: Vec<PreviewLineKind>,
    footer: Option<String>,
    rx: Option<Receiver<PreviewContentMsg>>,
    request_id: u64,
    pending: bool,
    cache: HashMap<PathBuf, PreviewCacheEntry>,
    native_area: Option<Rect>,
    native_last_key: Option<String>,
    image_rgb: Option<(Vec<u8>, u32, u32)>,
    image_png: Option<Vec<u8>>,
    pane_focus: PreviewPaneFocus,
}

/// Remote/mount picker state: active SSH/rclone mounts, the discovered remote
/// entries list, and the picker's selected row. Grouped out of `App`.
#[derive(Default)]
struct RemoteState {
    ssh_mounts: Vec<SshMount>,
    entries: Vec<RemoteEntry>,
    picker_selection: usize,
}

/// Tree-view state: per-directory expansion depths and the last +/- key tap
/// (for double-tap detection). Grouped out of `App`.
#[derive(Default)]
struct TreeState {
    expansion_levels: HashMap<PathBuf, usize>,
    last_tap: Option<(char, Instant)>,
}

/// Cached display widths for the group/owner metadata columns. Grouped out of
/// `App`.
struct MetaState {
    group_width: usize,
    owner_width: usize,
}

/// Header clock cache: the minute key it was last rendered for and the rendered
/// text. Grouped out of `App`.
#[derive(Default)]
struct HeaderClockState {
    minute_key: Option<i64>,
    text: String,
}

/// Shortcuts panel state: the selected action row and whether it is capturing a
/// replacement key. Grouped out of `App`.
#[derive(Default)]
struct ShortcutsPanelState {
    selected: usize,
    capture: bool,
}

/// Plugins panel state: the selected plugin row and whether it is capturing a
/// key to bind the selected plugin's `entry()`. Grouped out of `App`.
#[derive(Default)]
struct PluginsPanelState {
    selected: usize,
    key_capture: bool,
}

/// All state for the built-in incremental search overlay: candidate/result
/// lists, the async candidate + content scan channels, regex mode, and the
/// content-scan limits menu. Grouped out of `App` (was ~18 `internal_search_*`
/// fields).
struct SearchState {
    candidates: Vec<PathBuf>,
    /// Candidate rel-paths that are symlinks, captured during the walk so the
    /// results overlay never has to stat paths while rendering.
    candidate_symlinks: HashSet<PathBuf>,
    results: Vec<InternalSearchResult>,
    selected: usize,
    scope: InternalSearchScope,
    candidates_rx: Option<Receiver<InternalSearchCandidatesMsg>>,
    candidates_scan_id: u64,
    candidates_pending: bool,
    candidates_truncated: bool,
    content_rx: Option<Receiver<InternalSearchContentMsg>>,
    content_request_id: u64,
    content_pending: bool,
    content_limit_note: Option<String>,
    content_limits: InternalSearchContentLimits,
    limits_menu_open: bool,
    limits_selected: usize,
    regex_mode: bool,
    regex: Option<Regex>,
    regex_error: Option<String>,
}

/// Inputs and progress for an in-flight archive create/extract job.
///
/// Note: this is distinct from `App::archive_mounts`, which tracks mounted
/// archive filesystems rather than a running create/extract operation.
#[derive(Default)]
struct ArchiveOperation {
    create_targets: Vec<PathBuf>,
    extract_targets: Vec<PathBuf>,
    rx: Option<Receiver<ArchiveProgressMsg>>,
    total_bytes: u64,
    done_bytes: u64,
    started_at: Option<Instant>,
    name: String,
}

/// App-global folder/size accounting: the recursive folder-size jobs and their
/// caches plus the current-directory total/free-space figures. Grouped out of
/// `App` so the size-tracking state lives together. The per-panel
/// `selected_total_size*` fields stay in [`PanelState`] (they are per-panel).
#[derive(Default)]
struct SizeState {
    folder_size_enabled: bool,
    folder_size_cache: HashMap<PathBuf, u64>,
    folder_size: AsyncJobState<FolderSizeMsg>,
    current_dir_total_size: AsyncJobState<CurrentDirTotalSizeMsg>,
    current_dir_total_size_pending: bool,
    current_dir_total_size_bytes: Option<u64>,
    current_dir_total_space_bytes: Option<u64>,
    current_dir_free_bytes: Option<u64>,
    recursive_mtime: AsyncJobState<RecursiveMtimeMsg>,
}

struct App {
    /// The left/main panel's state. In single-panel mode this is the only
    /// panel; in dual-panel mode it is the left half (mirrors [`right`]).
    left: PanelState,
    directory_selection: HashMap<PathBuf, usize>,
    archive_mounts: Vec<ArchiveMount>,
    mode: AppMode,
    clipboard: Vec<PathBuf>,
    folder_filter_visible: bool,
    input_buffer: String,
    input_cursor: usize,
    status_message: String,
    right_status_message: String,
    page_size: usize,
    /// Remote/mount picker state: active SSH mounts, the discovered remote
    /// entries list, and the picker's selected row.
    remote: RemoteState,
    copy: CopyOperation,
    /// Paste/move queue state and download bookkeeping, grouped out of `App`.
    transfer: TransferState,
    archive: ArchiveOperation,
    nerd_font_active: bool,
    filename_color_mode: FilenameColorMode,
    os_icon: Option<(&'static str, ratatui::style::Color)>,
    no_color: bool,
    show_icons: bool,
    /// Bookmarks overlay selection/edit state plus the cached bookmark slots.
    bookmarks: BookmarkUiState,
    /// Integrations panel: selection, enable/disable overrides, cached rows, the
    /// search box, and the pending brew-install prompt.
    integration: IntegrationUiState,
    /// Delete-confirmation overlay: captured targets, scroll, and button focus
    /// (including the bookmark-delete variant).
    confirm_delete: ConfirmDeleteState,
    /// Help overlay scroll + logo-image render state.
    help: HelpState,
    confirm_integration_install_button_focus: u8,
    /// Background git-info cache and in-flight probe channel/timestamp.
    git: GitInfoState,
    /// When true, the top-right header clock is replaced by the disk-usage pill
    /// (without the recursive folder-size prefix). Persisted to config.
    disable_clock: bool,
    size: SizeState,
    /// Tree-view per-directory expansion depths and the last +/- tap (for
    /// double-tap detection).
    tree: TreeState,
    sort_menu_selected: usize,
    panel_tab: u8,
    active_theme: ui::theme::ThemeId,
    /// Themes panel selection and its Nerd-Fonts/filename-colors/disable-clock
    /// toggle-row focus flags.
    themes: ThemePanelState,
    /// Selected row in the Settings panel (0=Provider, 1=Model, 2=API Key,
    /// 3=Auto commit).
    settings_selected: usize,
    /// Active key bindings for Browsing-mode commands (defaults overlaid with
    /// the persisted `shortcut_*` overrides).
    keymap: util::keymap::KeyMap,
    /// The Lua plugin runtime (loaded plugins, their key bindings, hooks and
    /// in-flight `sb.spawn` jobs).
    plugins: plugin::runtime::PluginRuntime,
    /// Shortcuts panel: selected action row and whether it is capturing a key.
    shortcuts_panel: ShortcutsPanelState,
    /// Plugins panel: selected plugin row and whether it is capturing a key to
    /// bind the selected plugin's `entry()`.
    plugins_panel: PluginsPanelState,
    /// AI commit-message settings and runtime: provider/model/key edit buffers,
    /// stored per-provider keys, the auto-commit flag, the in-flight
    /// commit-request + key-validation channels, and key-validation state.
    ai: AiState,
    /// AI organize-plan request channel, the proposed plan, its target dir, and
    /// the review overlay's scroll/button state.
    organize: OrganizeState,
    search: SearchState,
    /// Per-file notes for the left (and, in dual-panel mode, right) directory:
    /// the loaded maps, async load channels + scan id, and the in-progress
    /// note-edit targets/dir.
    notes: NotesState,
    /// Cached column widths for the group/owner metadata columns.
    meta: MetaState,
    /// Header clock cache: the minute key it was rendered for and the text.
    header_clock: HeaderClockState,
    /// Set whenever state changes (user input, async result, clock tick) so the
    /// event loop repaints; cleared after each draw. Lets idle iterations skip
    /// the full render. See [`App::has_active_async_work`].
    needs_redraw: bool,
    /// SQLite preview overlay state: db path, table list + selection, rendered
    /// output lines, row limit, and any error.
    db_preview: DbPreviewState,
    view_mode: ViewMode,
    /// Preview pane content + scroll, the async content-load channel/request id,
    /// the rendered-content cache, and native/bitmap image render state.
    preview: PreviewState,
    // Click hit-zones for the footer shortcut pills (main footer + tabbed
    // overlay footers), rebuilt every render. Each entry is
    // (key event to synthesize, x_start, x_end_exclusive, y) in terminal cells.
    footer_shortcut_zones: Vec<(KeyEvent, u16, u16, u16)>,
    active_panel: DualPanelSide,
    right: PanelState,
}

const ZIP_BASED_EXTENSIONS: &[&str] = &[
    "zip", "jar", "war", "ear", "apk", "xpi", "crx", "cbz", "epub", "ipa",
    "odt", "ods", "odp", "odg", "odf", "ott", "ots", "otp", "sxw", "sxc",
    "sxi", "docx", "xlsx", "pptx", "vsix", "nupkg", "kmz", "whl",
];

fn env_flag_true(names: &[&str]) -> bool {
    for name in names {
        if let Ok(raw) = env::var(name) {
            let v = raw.trim();
            let is_true = v == "1" || v.eq_ignore_ascii_case("true");
            if !is_true && *name == "NO_COLOR" {
                // SAFETY: This runs during startup/list-mode initialization before any
                // worker threads are spawned, so mutating the process environment here
                // avoids races while ensuring falsey NO_COLOR values do not leak through
                // to downstream color handling.
                unsafe {
                    env::remove_var(name);
                }
            }
            return is_true;
        }
    }
    false
}


/// Returns (glyph, (r, g, b)) for well-known directory names, or None for generic folders.
fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        ui::cli::print_help();
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        ui::cli::print_version();
        return Ok(());
    }
    if let Err(message) = ui::cli::validate_cli_args(&args) {
        eprintln!("Error: {}", message);
        eprintln!("Run with --help to see supported usage.");
        return Ok(());
    }
    if let Some(list_args) = ui::cli::parse_list_mode_args(&args) {
        if !list_args.include_hidden && list_args.tree_depth.is_none()
            && let Some(path) = list_args.path {
                let target = PathBuf::from(path);
                if target.is_file() {
                    return App::open_path_in_view_mode(&target, true);
                }
            }
        return ui::cli::list_current_directory(
            list_args.include_hidden,
            list_args.include_total_size,
            list_args.tree_depth,
            list_args.path,
        );
    }

    if let Some((mode, path)) = ui::cli::parse_direct_file_mode_args(&args) {
        let target = PathBuf::from(path);
        if target.is_file() {
            return match mode {
                ui::cli::DirectFileMode::ViewNoPager => App::open_path_in_view_mode(&target, false),
                ui::cli::DirectFileMode::ViewWithPager => App::open_path_in_view_mode(&target, true),
                ui::cli::DirectFileMode::Edit => App::open_path_in_editor_cli(&target),
            };
        } else if target.is_dir() && matches!(mode, ui::cli::DirectFileMode::Edit) {
            // If -e is used with a directory, open the TUI file manager in that directory
            let _ = env::set_current_dir(&target);
        }
    }

    // If a single argument is provided that is a directory, list it like -l
    if args.len() == 1 && !args[0].starts_with('-')
        && let Ok(target) = PathBuf::from(&args[0]).canonicalize()
            && target.is_dir() {
                return ui::cli::list_current_directory(false, false, None, Some(&args[0]));
            }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mut app = App::new()?;

    run::run_tui(&mut terminal, &mut app)?;
    // Fire plugin on_quit hooks; the UI is going away, so their effects are
    // discarded (side effects like writing files still happen in Lua).
    if app.plugins.wants_hook(plugin::runtime::Hook::Quit) {
        let ctx = app.plugin_ctx();
        let _ = app.plugins.run_hook(plugin::runtime::Hook::Quit, &ctx);
    }
    app.cleanup_archive_mounts();
    app.cleanup_ssh_mounts();
    // Best-effort: exiting matters more than persisting cosmetic view state.
    let _ = util::config::SbPersistConfig::update(|persist| {
        persist.view_mode = format!("{:?}", app.view_mode);
        persist.current_theme = ui::theme::theme_name(app.active_theme).to_string();
        persist.disabled_integrations = app
            .integration.overrides
            .iter()
            .filter_map(|(k, &v)| if !v { Some(k.clone()) } else { None })
            .collect();
    });
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        TermClear(ClearType::All),
        MoveTo(0, 0)
    )?;
    // Best-effort: the optional shell `cd`-on-exit snippet (see README) reads this
    // path. Intentionally fire-and-forget — the app is already exiting and a
    // failed write only means the shell won't follow us into the last directory.
    // The path is a documented contract with that snippet; do not relocate it here
    // without updating the README integration.
    let last_path = util::config::last_path_file();
    if let Some(parent) = last_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(&last_path, app.active_panel_dir().to_string_lossy().as_bytes()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&last_path, std::fs::Permissions::from_mode(0o600));
        }
    }
    Ok(())
}