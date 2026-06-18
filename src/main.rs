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
mod app_archive;
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
mod app_preview;
mod app_remote;
mod app_rendering;
mod app_render_cache;
pub(crate) use app_render_cache::{EntryRenderCache, EntryRenderConfig};
mod app_model;
pub(crate) use app_model::*;
mod app_search;
mod app_sizes;
mod app_sort;
mod app_shell;
mod app_sqlite;
mod app_transfer;
mod ui;
mod util;
mod run;

use integration::rows::IntegrationRow;

struct PanelState {
    dir: PathBuf,
    entries: Vec<fs::DirEntry>,
    entry_render_cache: Vec<EntryRenderCache>,
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
    selected_total_size_rx: Option<Receiver<SelectedTotalSizeMsg>>,
    selected_total_size_scan_id: u64,
    selected_total_size_pending: bool,
    selected_total_size_bytes: Option<u64>,
    selected_total_size_items: usize,
}

struct App {
    current_dir: PathBuf,
    entries: Vec<fs::DirEntry>,
    entry_render_cache: Vec<EntryRenderCache>,
    selected_index: usize,
    marked_indices: HashSet<usize>,
    directory_selection: HashMap<PathBuf, usize>,
    archive_mounts: Vec<ArchiveMount>,
    mode: AppMode,
    table_state: TableState,
    show_hidden: bool,
    clipboard: Vec<PathBuf>,
    paste_queue: VecDeque<PathBuf>,
    paste_current_src: Option<PathBuf>,
    paste_move_mode: bool,
    paste_target_dir: Option<PathBuf>,
    path_input_filter: Option<PathInputFilter>,
    folder_filter_visible: bool,
    input_buffer: String,
    input_cursor: usize,
    status_message: String,
    right_status_message: String,
    page_size: usize,
    ssh_mounts: Vec<SshMount>,
    remote_entries: Vec<RemoteEntry>,
    ssh_picker_selection: usize,
    copy_rx: Option<Receiver<CopyProgressMsg>>,
    copy_total_rx: Option<Receiver<u64>>,
    copy_total_bytes: u64,
    copy_done_bytes: u64,
    copy_job_total_bytes: u64,
    copy_done_before_job: u64,
    copy_started_at: Option<Instant>,
    copy_item_name: String,
    copy_current_src: Option<PathBuf>,
    copy_from_remote: bool,
    download_rx: Option<Receiver<DownloadProgressMsg>>,
    download_pending_url: Option<String>,
    download_pending_name: Option<String>,
    download_resume_input: Option<String>,
    download_active_name: String,
    paste_total_items: usize,
    paste_ok_items: usize,
    paste_failed_items: usize,
    archive_create_targets: Vec<PathBuf>,
    archive_extract_targets: Vec<PathBuf>,
    archive_rx: Option<Receiver<ArchiveProgressMsg>>,
    archive_total_bytes: u64,
    archive_done_bytes: u64,
    archive_started_at: Option<Instant>,
    archive_name: String,
    nerd_font_active: bool,
    filename_color_mode: FilenameColorMode,
    os_icon: Option<(&'static str, ratatui::style::Color)>,
    no_color: bool,
    show_icons: bool,
    integration_selected: usize,
    bookmark_selected: usize,
    bookmark_edit_idx: usize,
    bookmark_delete_idx: usize,
    confirm_delete_bookmark_button_focus: u8,
    integration_overrides: HashMap<String, bool>,
    integration_rows_cache: Vec<IntegrationRow>,
    integration_search_active: bool,
    integration_search_query: String,
    integration_install_key: Option<String>,
    integration_install_package: Option<String>,
    integration_install_brew_path: Option<String>,
    help_scroll_offset: u16,
    help_max_offset: u16,
    confirm_delete_scroll_offset: u16,
    confirm_delete_max_offset: u16,
    file_list_scroll_dragging: bool,
    file_list_scroll_grab_offset: u16,
    confirm_delete_button_focus: u8,
    confirm_integration_install_button_focus: u8,
    git_info_cache: Option<GitInfoCache>,
    git_info_rx: Option<Receiver<(PathBuf, Option<(String, bool, Option<(String, u64)>)>)>>,
    git_last_check_at: Option<Instant>,
    folder_size_enabled: bool,
    folder_size_cache: HashMap<PathBuf, u64>,
    folder_size_rx: Option<Receiver<FolderSizeMsg>>,
    folder_size_scan_id: u64,
    tree_expansion_levels: HashMap<PathBuf, usize>,
    tree_last_tap: Option<(char, Instant)>,
    main_list_last_click: Option<(PathBuf, usize, Instant)>,
    tree_row_prefixes: Vec<String>,
    current_dir_total_size_rx: Option<Receiver<CurrentDirTotalSizeMsg>>,
    current_dir_total_size_scan_id: u64,
    current_dir_total_size_pending: bool,
    current_dir_total_size_bytes: Option<u64>,
    current_dir_total_space_bytes: Option<u64>,
    current_dir_free_bytes: Option<u64>,
    recursive_mtime_rx: Option<Receiver<RecursiveMtimeMsg>>,
    recursive_mtime_scan_id: u64,
    selected_total_size_rx: Option<Receiver<SelectedTotalSizeMsg>>,
    selected_total_size_scan_id: u64,
    selected_total_size_pending: bool,
    selected_total_size_bytes: Option<u64>,
    selected_total_size_items: usize,
    sort_mode: SortMode,
    sort_menu_selected: usize,
    panel_tab: u8,
    active_theme: ui::theme::ThemeId,
    theme_selected: usize,
    /// True when the Themes panel's "Nerd Fonts" toggle row is the selected row
    /// (rather than one of the theme rows).
    theme_panel_nerd_selected: bool,
    /// True when the Themes panel's "Filename colors" toggle row is the selected
    /// row (rather than the Nerd Fonts row or one of the theme rows).
    theme_panel_color_selected: bool,
    internal_search_candidates: Vec<PathBuf>,
    internal_search_results: Vec<InternalSearchResult>,
    internal_search_selected: usize,
    internal_search_scope: InternalSearchScope,
    internal_search_candidates_rx: Option<Receiver<InternalSearchCandidatesMsg>>,
    internal_search_candidates_scan_id: u64,
    internal_search_candidates_pending: bool,
    internal_search_candidates_truncated: bool,
    internal_search_content_rx: Option<Receiver<InternalSearchContentMsg>>,
    internal_search_content_request_id: u64,
    internal_search_content_pending: bool,
    internal_search_content_limit_note: Option<String>,
    internal_search_content_limits: InternalSearchContentLimits,
    internal_search_limits_menu_open: bool,
    internal_search_limits_selected: usize,
    internal_search_regex_mode: bool,
    internal_search_regex: Option<Regex>,
    internal_search_regex_error: Option<String>,
    notes_by_name: HashMap<String, String>,
    notes_rx: Option<Receiver<NotesLoadMsg>>,
    notes_scan_id: u64,
    notes_loaded_for: Option<PathBuf>,
    right_notes_by_name: HashMap<String, String>,
    right_notes_rx: Option<Receiver<NotesLoadMsg>>,
    right_notes_loaded_for: Option<PathBuf>,
    note_edit_targets: Vec<String>,
    note_edit_dir: PathBuf,
    meta_group_width: usize,
    meta_owner_width: usize,
    header_clock_minute_key: Option<i64>,
    header_clock_text: String,
    db_preview_path: Option<PathBuf>,
    db_preview_tables: Vec<String>,
    db_preview_selected: usize,
    db_preview_output_lines: Vec<String>,
    db_preview_row_limit: usize,
    db_preview_error: Option<String>,
    view_mode: ViewMode,
    preview_scroll_offset: usize,
    preview_target_path: Option<PathBuf>,
    preview_lines: Vec<String>,
    preview_line_kinds: Vec<PreviewLineKind>,
    preview_footer: Option<String>,
    preview_rx: Option<Receiver<PreviewContentMsg>>,
    preview_request_id: u64,
    preview_pending: bool,
    preview_cache: HashMap<PathBuf, (Vec<String>, Vec<PreviewLineKind>, Option<String>)>,
    preview_native_area: Option<Rect>,
    preview_native_last_key: Option<String>,
    preview_image_rgb: Option<(Vec<u8>, u32, u32)>,
    preview_image_png: Option<Vec<u8>>,
    preview_pane_focus: PreviewPaneFocus,
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

impl App {
    fn open_path_in_editor_cli(path: &PathBuf) -> io::Result<()> {
        // Check if file is binary and use appropriate editor
        if Self::is_binary_file(path) {
            // Try hexedit first (interactive binary editor)
            if Self::integration_probe("hexedit").0 {
                let _ = Command::new("hexedit").arg(path).status();
            }
            // Fall back to hexyl with less paging if hexedit is not available
            if Self::integration_probe("hexyl").0 {
                let mut cmd = Command::new("hexyl");
                cmd.arg(path);
                let _ = crate::util::command::pipe_to_pager(cmd);
                return Ok(());
            }
        }

        // For text files or if no binary editors available, use regular editor
        let editor = crate::util::command::editor_command();
        let _ = Command::new(editor).arg(path).status()?;
        Ok(())
    }

    fn new() -> io::Result<Self> {
        let current_dir = app_init::init_current_dir()?;
        let internal_search_content_limits = Self::internal_search_content_limits();
        let mut app = Self {
            current_dir,
            entries: Vec::new(),
            entry_render_cache: Vec::new(),
            selected_index: 0,
            marked_indices: HashSet::new(),
            directory_selection: HashMap::new(),
            archive_mounts: Vec::new(),
            mode: AppMode::Browsing,
            table_state: TableState::default(),
            show_hidden: false,
            clipboard: Vec::new(),
            paste_queue: VecDeque::new(),
            paste_current_src: None,
            paste_move_mode: false,
            paste_target_dir: None,
            path_input_filter: None,
            folder_filter_visible: false,
            input_buffer: String::new(),
            input_cursor: 0,
            status_message: String::new(),
            right_status_message: String::new(),
            page_size: 20,
            ssh_mounts: Vec::new(),
            remote_entries: Vec::new(),
            ssh_picker_selection: 0,
            copy_rx: None,
            copy_total_rx: None,
            copy_total_bytes: 0,
            copy_done_bytes: 0,
            copy_job_total_bytes: 0,
            copy_done_before_job: 0,
            copy_started_at: None,
            copy_item_name: String::new(),
            copy_current_src: None,
            copy_from_remote: false,
            download_rx: None,
            download_pending_url: None,
            download_pending_name: None,
            download_resume_input: None,
            download_active_name: String::new(),
            paste_total_items: 0,
            paste_ok_items: 0,
            paste_failed_items: 0,
            archive_create_targets: Vec::new(),
            archive_extract_targets: Vec::new(),
            archive_rx: None,
            archive_total_bytes: 0,
            archive_done_bytes: 0,
            archive_started_at: None,
            archive_name: String::new(),
            nerd_font_active: env::var("NERD_FONT_ACTIVE").map(|v| v == "1").unwrap_or(false),
            filename_color_mode: FilenameColorMode::Full,
            os_icon: ui::icons::os_nerd_icon().map(|(g, _)| {
                (g, ui::theme::theme_spec(ui::theme::ThemeId::original()).icon_os)
            }),
            no_color: env_flag_true(&["NO_COLOR"]),
            show_icons: env::var("TERMINAL_ICONS").map(|v| v != "0").unwrap_or(true),
            integration_selected: 0,
            bookmark_selected: 0,
            bookmark_edit_idx: 0,
            bookmark_delete_idx: 0,
            confirm_delete_bookmark_button_focus: 0,
            integration_overrides: HashMap::new(),
            integration_rows_cache: Vec::new(),
            integration_search_active: false,
            integration_search_query: String::new(),
            integration_install_key: None,
            integration_install_package: None,
            integration_install_brew_path: None,
            help_scroll_offset: 0,
            help_max_offset: 0,
            confirm_delete_scroll_offset: 0,
            confirm_delete_max_offset: 0,
            file_list_scroll_dragging: false,
            file_list_scroll_grab_offset: 0,
            confirm_delete_button_focus: 0,
            confirm_integration_install_button_focus: 0,
            git_info_cache: None,
            git_info_rx: None,
            git_last_check_at: None,
            folder_size_enabled: false,
            folder_size_cache: HashMap::new(),
            folder_size_rx: None,
            folder_size_scan_id: 0,
            tree_expansion_levels: HashMap::new(),
            tree_last_tap: None,
            main_list_last_click: None,
            tree_row_prefixes: Vec::new(),
            current_dir_total_size_rx: None,
            current_dir_total_size_scan_id: 0,
            current_dir_total_size_pending: false,
            current_dir_total_size_bytes: None,
            current_dir_total_space_bytes: None,
            current_dir_free_bytes: None,
            recursive_mtime_rx: None,
            recursive_mtime_scan_id: 0,
            selected_total_size_rx: None,
            selected_total_size_scan_id: 0,
            selected_total_size_pending: false,
            selected_total_size_bytes: None,
            selected_total_size_items: 0,
            sort_mode: SortMode::NameAsc,
            sort_menu_selected: 0,
            panel_tab: 0,
            active_theme: ui::theme::ThemeId::original(),
            theme_selected: 0,
            theme_panel_nerd_selected: false,
            theme_panel_color_selected: false,
            internal_search_candidates: Vec::new(),
            internal_search_results: Vec::new(),
            internal_search_selected: 0,
            internal_search_scope: InternalSearchScope::Filename,
            internal_search_candidates_rx: None,
            internal_search_candidates_scan_id: 0,
            internal_search_candidates_pending: false,
            internal_search_candidates_truncated: false,
            internal_search_content_rx: None,
            internal_search_content_request_id: 0,
            internal_search_content_pending: false,
            internal_search_content_limit_note: None,
            internal_search_content_limits,
            internal_search_limits_menu_open: false,
            internal_search_limits_selected: 0,
            internal_search_regex_mode: false,
            internal_search_regex: None,
            internal_search_regex_error: None,
            notes_by_name: HashMap::new(),
            notes_rx: None,
            notes_scan_id: 0,
            notes_loaded_for: None,
            right_notes_by_name: HashMap::new(),
            right_notes_rx: None,
            right_notes_loaded_for: None,
            note_edit_targets: Vec::new(),
            note_edit_dir: PathBuf::new(),
            meta_group_width: 1,
            meta_owner_width: 1,
            header_clock_minute_key: None,
            header_clock_text: String::new(),
            db_preview_path: None,
            db_preview_tables: Vec::new(),
            db_preview_selected: 0,
            db_preview_output_lines: Vec::new(),
            db_preview_row_limit: 8,
            db_preview_error: None,
            view_mode: ViewMode::Normal,
            preview_scroll_offset: 0,
            preview_target_path: None,
            preview_lines: Vec::new(),
            preview_line_kinds: Vec::new(),
            preview_footer: None,
            preview_rx: None,
            preview_request_id: 0,
            preview_pending: false,
            preview_cache: HashMap::new(),
            preview_native_area: None,
            preview_native_last_key: None,
            preview_image_rgb: None,
            preview_image_png: None,
            preview_pane_focus: PreviewPaneFocus::Folder,
            active_panel: DualPanelSide::Left,
            right: PanelState {
                dir: PathBuf::new(),
                entries: Vec::new(),
                entry_render_cache: Vec::new(),
                selected_index: 0,
                marked_indices: HashSet::new(),
                table_state: TableState::default(),
                sort_mode: SortMode::NameAsc,
                show_hidden: false,
                folder_filter: None,
                list_scroll_dragging: false,
                list_scroll_grab_offset: 0,
                list_last_click: None,
                tree_row_prefixes: Vec::new(),
                selected_total_size_rx: None,
                selected_total_size_scan_id: 0,
                selected_total_size_pending: false,
                selected_total_size_bytes: None,
                selected_total_size_items: 0,
            },
        };
        app.refresh_header_clock_if_needed();
        app.refresh_entries()?;
        app.request_notes_for_current_dir_once();
        app.request_notes_for_right_panel_once();
        app.request_git_info_for_current_dir_once();
        // Restore persisted view mode from ~/.config/sb/config
        let persist = util::config::SbPersistConfig::load();
        match persist.view_mode.as_str() {
            "Preview" => app.cycle_view_mode(),
            "DualPanel" => {
                app.cycle_view_mode();
                app.cycle_view_mode();
            }
            _ => {}
        }
        // Persisted Nerd Font choice overrides the NERD_FONT_ACTIVE env var.
        // Applied before set_active_theme so the render-cache rebuild uses the
        // correct glyph mode.
        if let Some(nf) = persist.nerd_font {
            app.nerd_font_active = nf;
        }
        // Persisted filename-color mode; applied before set_active_theme so the
        // first render-cache build uses it.
        app.filename_color_mode = persist.filename_color_mode;
        app.set_active_theme(ui::theme::theme_by_name(&persist.current_theme));
        for key in &persist.disabled_integrations {
            app.integration_overrides.insert(key.clone(), false);
        }
        Ok(app)
    }

    fn refresh_header_clock_if_needed(&mut self) {
        let now = Local::now();
        let minute_key = now.timestamp().div_euclid(60);
        if self.header_clock_minute_key == Some(minute_key) {
            return;
        }
        self.header_clock_minute_key = Some(minute_key);
        self.header_clock_text = now.format("%Y-%m-%d %H:%M").to_string();
    }

    fn search_spans_with_ranges(
        text: &str,
        ranges: &[(usize, usize)],
        base_style: Style,
        match_style: Style,
    ) -> Vec<Span<'static>> {
        ui::search::search_spans_with_ranges(text, ranges, base_style, match_style)
    }

    fn refresh_entries_or_status(&mut self) -> bool {
        match self.refresh_entries() {
            Ok(()) => {
                if self.copy_rx.is_none() && self.archive_rx.is_none() {
                    self.status_message.clear();
                }
                true
            }
            Err(e) => {
                self.set_status(format!("refresh failed: {}", e));
                false
            }
        }
    }

    fn try_enter_dir(&mut self, target: PathBuf) {
        let previous_dir = self.current_dir.clone();
        let previous_filter = self.path_input_filter.clone();
        let changed_dir = target != previous_dir;
        self.remember_current_selection();
        self.current_dir = target;
        if changed_dir {
            self.path_input_filter = None;
            self.folder_filter_visible = false;
        }
        if !self.refresh_entries_or_status() {
            self.current_dir = previous_dir;
            self.path_input_filter = previous_filter;
        } else {
            self.restore_selection_for_current_dir();
            self.request_git_info_for_current_dir_once();
        }
    }

    fn active_panel_dir(&self) -> PathBuf {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.dir.clone()
        } else {
            self.current_dir.clone()
        }
    }

    pub(crate) fn active_selected_entry_path(&self) -> Option<PathBuf> {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.entries.get(self.right.selected_index).map(|e| e.path())
        } else {
            self.entries.get(self.selected_index).map(|e| e.path())
        }
    }

    pub(crate) fn active_entries_empty(&self) -> bool {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.entries.is_empty()
        } else {
            self.entries.is_empty()
        }
    }

    fn try_enter_dir_on_active_panel(&mut self, target: PathBuf) {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            // Changing directory clears any active folder filter on the right panel.
            if target != self.right.dir {
                self.right.folder_filter = None;
                self.folder_filter_visible = false;
            }
            self.right.dir = target;
            if self.refresh_right_panel_entries().is_err() {
                self.set_status("refresh failed");
            }
        } else {
            self.try_enter_dir(target);
        }
    }

    /// Whether the folder-filter box currently applies to the left/main panel.
    fn folder_filter_on_left(&self) -> bool {
        self.folder_filter_visible
            && !(self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right)
    }

    /// Whether the folder-filter box currently applies to the right panel.
    fn folder_filter_on_right(&self) -> bool {
        self.folder_filter_visible
            && self.is_dual_panel_mode()
            && self.active_panel == DualPanelSide::Right
    }

    /// Open the folder-filter box on the active panel, seeding it with the
    /// current filter pattern (if any) so it can be edited in place.
    fn begin_folder_filter(&mut self) {
        let current = if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.folder_filter.as_ref().map(|f| f.pattern.clone())
        } else {
            self.path_input_filter.as_ref().map(|f| f.pattern.clone())
        }
        .unwrap_or_default();
        self.folder_filter_visible = true;
        self.begin_input_edit(AppMode::FolderFilter, current);
    }

    /// Re-derive and apply the folder filter from the current input buffer to
    /// the active panel, refreshing its listing live.
    fn apply_folder_filter_live(&mut self) {
        let pattern = self.input_buffer.trim().to_string();
        let new_filter = if pattern.is_empty() {
            None
        } else {
            let candidate = PathInputFilter {
                mode: PathFilterMode::Contains,
                pattern,
            };
            match Self::build_path_filter_regex(&candidate) {
                Ok(_) => Some(candidate),
                Err(err) => {
                    self.set_status(format!("invalid filter regex: {}", err));
                    None
                }
            }
        };
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.folder_filter = new_filter;
            if self.refresh_right_panel_entries().is_err() {
                self.set_status("refresh failed");
            }
        } else {
            self.path_input_filter = new_filter;
            self.refresh_entries_or_status();
        }
    }

    /// Hide the folder-filter box and clear the filter on the active panel.
    fn clear_folder_filter(&mut self) {
        self.folder_filter_visible = false;
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            if self.right.folder_filter.take().is_some()
                && self.refresh_right_panel_entries().is_err()
            {
                self.set_status("refresh failed");
            }
        } else if self.path_input_filter.take().is_some() {
            self.refresh_entries_or_status();
        }
    }




    fn create_temp_selection_path(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        env::temp_dir().join(format!("{}_{}_{}.txt", prefix, std::process::id(), stamp))
    }

    fn remote_mount_for_path(&self, path: &PathBuf) -> Option<&SshMount> {
        self.ssh_mounts
            .iter()
            .filter(|mount| path == &mount.mount_path || path.starts_with(&mount.mount_path))
            .max_by_key(|mount| mount.mount_path.components().count())
    }

    fn display_path_for(&self, path: &PathBuf) -> String {
        let Some(mount) = self.remote_mount_for_path(path) else {
            let path_str = path.to_string_lossy().into_owned();
            if let Ok(home) = env::var("HOME") {
                if path_str == home {
                    return "~".to_string();
                }
                let home_prefix = format!("{}/", home);
                if let Some(rest) = path_str.strip_prefix(&home_prefix) {
                    return format!("~/{}", rest);
                }
            }
            return path_str;
        };

        let rel = path
            .strip_prefix(&mount.mount_path)
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();

        if rel.is_empty() {
            return mount.remote_root.clone();
        }

        if mount.remote_root == "/" {
            format!("/{}", rel)
        } else if mount.remote_root.ends_with('/') {
            format!("{}{}", mount.remote_root, rel)
        } else {
            format!("{}/{}", mount.remote_root, rel)
        }
    }

    fn remember_current_selection(&mut self) {
        self.directory_selection
            .insert(self.current_dir.clone(), self.selected_index);
    }

    fn restore_selection_for_current_dir(&mut self) {
        if self.entries.is_empty() {
            self.selected_index = 0;
            self.table_state.select(None);
            return;
        }

        let index = self
            .directory_selection
            .get(&self.current_dir)
            .copied()
            .unwrap_or(0)
            .min(self.entries.len() - 1);
        self.selected_index = index;
        self.table_state.select(Some(index));
    }

    fn select_entry_named(&mut self, name: &str) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.file_name().to_string_lossy() == name)
        {
            self.selected_index = index;
            self.table_state.select(Some(index));
        }
    }

    fn select_right_entry_named(&mut self, name: &str) {
        if let Some(index) = self
            .right.entries
            .iter()
            .position(|entry| entry.file_name().to_string_lossy() == name)
        {
            self.right.selected_index = index;
            self.right.table_state.select(Some(index));
        }
    }

    fn try_enter_parent_dir(&mut self) {
        let child_name = self
            .current_dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());

        if let Some(parent) = self.current_dir.parent() {
            self.try_enter_dir(parent.to_path_buf());
            if let Some(name) = child_name {
                self.select_entry_named(&name);
            }
        }
    }

    fn resolve_input_path(&self, raw: &str) -> PathBuf {
        let trimmed = raw.trim();
        if let Some(rest) = trimmed.strip_prefix("~/")
            && let Ok(home) = env::var("HOME") {
                return PathBuf::from(home).join(rest);
            }
        if trimmed == "~"
            && let Ok(home) = env::var("HOME") {
                return PathBuf::from(home);
            }

        let candidate = PathBuf::from(trimmed);
        if candidate.is_absolute() {
            candidate
        } else {
            self.current_dir.join(candidate)
        }
    }

    fn apply_path_input(&mut self) {
        let raw_input = self.input_buffer.trim().to_string();
        let target = self.resolve_input_path(&raw_input);
        if target.is_dir() {
            // No-op: same directory, no filter to clear — skip refresh to avoid timestamp flicker
            if target == self.current_dir && self.path_input_filter.is_none() {
                self.mode = AppMode::Browsing;
                self.clear_input_edit();
                return;
            }
            self.path_input_filter = None;
            self.try_enter_dir(target);
            self.mode = AppMode::Browsing;
            self.clear_input_edit();
            return;
        }

        let Some((base_raw, filter)) = Self::parse_path_filter_suffix(&raw_input) else {
            self.set_status("path is not a directory");
            return;
        };

        if let Err(err) = Self::build_path_filter_regex(&filter) {
            self.set_status(format!("invalid path filter regex: {}", err));
            return;
        }

        let base_target = self.resolve_input_path(&base_raw);
        if !base_target.is_dir() {
            self.set_status("path is not a directory");
            return;
        }

        self.try_enter_dir(base_target);
        self.path_input_filter = Some(filter);
        self.refresh_entries_or_status();
        self.mode = AppMode::Browsing;
        self.clear_input_edit();
    }

    fn input_cursor_line_col(&self) -> (usize, usize) {
        let mut line = 0usize;
        let mut col = 0usize;
        for ch in self.input_buffer.chars().take(self.input_cursor) {
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn active_input_line_text(&self) -> String {
        let (line_idx, _) = self.input_cursor_line_col();
        self.input_buffer
            .split('\n')
            .nth(line_idx)
            .unwrap_or_default()
            .to_string()
    }

    fn create_entries_from_input(&mut self, default_is_dir: bool) {
        let target_dir = self.active_panel_dir();
        let mut specs: Vec<(String, bool)> = Vec::new();
        for raw_line in self.input_buffer.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            let (name, is_dir) = if let Some(rest) = line.strip_prefix('/') {
                (rest.trim().to_string(), true)
            } else {
                (line.to_string(), default_is_dir)
            };
            if !name.is_empty() {
                specs.push((name, is_dir));
            }
        }

        if specs.is_empty() {
            self.set_status("name cannot be empty");
            return;
        }

        let mut created: Vec<String> = Vec::new();
        let mut failed = 0usize;
        let mut first_error: Option<String> = None;

        for (name, is_dir) in specs {
            let target = target_dir.join(&name);
            if target.exists() {
                failed += 1;
                if first_error.is_none() {
                    first_error = Some("target already exists".to_string());
                }
                continue;
            }

            let result = if is_dir {
                fs::create_dir(&target)
            } else {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .map(|_| ())
            };
            match result {
                Ok(()) => created.push(name),
                Err(e) => {
                    failed += 1;
                    if first_error.is_none() {
                        first_error = Some(format!("create failed: {}", e));
                    }
                }
            }
        }

        if created.is_empty() {
            self.set_status(first_error.unwrap_or_else(|| "create failed".to_string()));
            return;
        }

        let last_created = created.last().cloned();
        self.mode = AppMode::Browsing;
        self.clear_input_edit();
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            if self.refresh_right_panel_entries().is_err() {
                self.set_status("refresh failed");
                return;
            }
            if let Some(name) = last_created
                && let Some(index) = self
                    .right.entries
                    .iter()
                    .position(|entry| entry.file_name().to_string_lossy() == name)
                {
                    self.right.selected_index = index;
                    self.right.table_state.select(Some(index));
                }
        } else {
            self.refresh_entries_or_status();
            if let Some(name) = last_created {
                self.select_entry_named(&name);
            }
        }

        if failed == 0 {
            self.set_status(format!("created {} item(s)", created.len()));
        } else {
            self.set_status(format!("created {} item(s), {} failed", created.len(), failed));
        }
    }

    fn paste_clipboard_at_input_cursor(&mut self) {
        let Some((raw, _backend)) = self.read_system_clipboard_text() else {
            self.set_status("no clipboard backend available (wl-copy/xclip/xsel/pbcopy)");
            return;
        };
        let normalized: String = raw
            .trim_end()
            .replace('\r', "")
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if normalized.is_empty() {
            self.set_status("clipboard is empty");
            return;
        }
        self.input_insert_str(&normalized);
    }

    fn refresh_entries(&mut self) -> io::Result<()> {
        let folder_size_cache = if self.folder_size_enabled {
            Some(&self.folder_size_cache)
        } else {
            None
        };
        let mut tree_row_prefixes = Vec::new();
        let mut entries: Vec<_> = if !self.tree_expansion_levels.is_empty() {
            let rows = ui::tree::collect_tree_rows_with_expansions(
                &self.current_dir,
                self.show_hidden,
                self.sort_mode,
                folder_size_cache,
                &self.tree_expansion_levels,
            )?;
            tree_row_prefixes = rows.iter().map(|row| row.prefix.clone()).collect();
            rows.into_iter().map(|row| row.entry).collect()
        } else {
            let mut direct_entries: Vec<_> = fs::read_dir(&self.current_dir)?
                .filter_map(|res| res.ok())
                .filter(|e| self.show_hidden || !crate::util::classify::is_hidden_entry(e))
                .collect();
            Self::sort_entries_by_mode(&mut direct_entries, self.sort_mode, folder_size_cache);
            direct_entries
        };
        if let Some(filter) = self.path_input_filter.as_ref() {
            let filter_regex = Self::build_path_filter_regex(filter)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
            if !self.tree_expansion_levels.is_empty() {
                let mut filtered_entries = Vec::new();
                let mut filtered_prefixes = Vec::new();
                for (entry, prefix) in entries.into_iter().zip(tree_row_prefixes.into_iter()) {
                    let name = crate::util::classify::entry_name(&entry);
                    if Self::entry_name_matches_path_filter(&name, &filter_regex) {
                        filtered_entries.push(entry);
                        filtered_prefixes.push(prefix);
                    }
                }
                entries = filtered_entries;
                tree_row_prefixes = filtered_prefixes;
            } else {
                entries.retain(|entry| {
                    let name = crate::util::classify::entry_name(entry);
                    Self::entry_name_matches_path_filter(&name, &filter_regex)
                });
            }
        }
        self.entries = entries;
        self.tree_row_prefixes = if !self.tree_expansion_levels.is_empty() {
            tree_row_prefixes
        } else {
            vec![String::new(); self.entries.len()]
        };
        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
            filename_color_mode: self.filename_color_mode,
        };
        let uid_cache = App::build_uid_cache(&self.entries);
        let gid_cache = App::build_gid_cache(&self.entries);
            self.entry_render_cache = self.entries.iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        self.apply_cached_folder_size_columns();
        self.refresh_meta_identity_widths();
        self.refresh_current_dir_free_space();
        self.folder_size_scan_id = self.folder_size_scan_id.wrapping_add(1);
        self.folder_size_rx = None;
        self.recursive_mtime_rx = None;
        self.clear_current_dir_total_size_state();
        self.clear_selected_total_size_state();
        self.marked_indices.clear();
        
        if self.entries.is_empty() {
            self.selected_index = 0;
            self.table_state.select(None);
        } else {
            self.selected_index = self.selected_index.min(self.entries.len() - 1);
            self.table_state.select(Some(self.selected_index));
        }

        if self.folder_size_enabled {
            self.start_folder_size_scan();
            self.start_current_dir_total_size_scan();
        }
        self.start_recursive_mtime_scan();
        self.request_notes_for_current_dir_once();
        self.request_notes_for_right_panel_once();
        Ok(())
    }

    fn request_notes_for_right_panel_once(&mut self) {
        // No right-panel directory yet (e.g. before dual-panel mode is entered).
        if self.right.dir.as_os_str().is_empty() {
            return;
        }
        if self.right_notes_loaded_for.as_ref().map(|p| p == &self.right.dir).unwrap_or(false) {
            return;
        }
        let dir = self.right.dir.clone();
        self.right_notes_by_name.clear();
        self.right_notes_rx = Some(util::background::spawn_worker(move |tx| {
            let notes = App::load_notes_map_for_dir(&dir);
            let _ = tx.send(NotesLoadMsg::Finished(0, dir, notes));
        }));
    }

    fn pump_right_notes_progress(&mut self) {
        let Some(rx) = self.right_notes_rx.take() else {
            return;
        };
        let mut keep_rx = true;
        loop {
            match rx.try_recv() {
                Ok(NotesLoadMsg::Finished(_, path, notes)) => {
                    if path == self.right.dir {
                        self.right_notes_by_name = notes;
                        self.right_notes_loaded_for = Some(path);
                        keep_rx = false;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => { keep_rx = false; break; }
            }
        }
        if keep_rx {
            self.right_notes_rx = Some(rx);
        }
    }

    fn delete_targets(&self) -> Vec<PathBuf> {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            if !self.right.marked_indices.is_empty() {
                self.right.entries
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| self.right.marked_indices.contains(i))
                    .map(|(_, e)| e.path())
                    .collect()
            } else {
                self.right.entries
                    .get(self.right.selected_index)
                    .map(|e| e.path())
                    .into_iter()
                    .collect()
            }
        } else if !self.marked_indices.is_empty() {
            self.entries
                .iter()
                .enumerate()
                .filter(|(i, _)| self.marked_indices.contains(i))
                .map(|(_, e)| e.path())
                .collect()
        } else {
            self.entries
                .get(self.selected_index)
                .map(|e| e.path())
                .into_iter()
                .collect()
        }
    }

    fn begin_confirm_delete(&mut self) {
        self.confirm_delete_scroll_offset = 0;
        self.confirm_delete_max_offset = 0;
        self.confirm_delete_button_focus = 0;
        self.mode = AppMode::ConfirmDelete;
    }

    fn confirm_delete_selected_targets(&mut self) {
        let to_delete = self.delete_targets();
        let mut failed = 0usize;
        let mut last_err: Option<String> = None;
        for path in to_delete {
            let result = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            if let Err(e) = result {
                failed += 1;
                last_err = Some(e.to_string());
            }
        }
        self.mode = AppMode::Browsing;
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            if self.refresh_right_panel_entries().is_err() {
                self.set_status("refresh failed");
            }
        } else {
            self.refresh_entries_or_status();
        }
        // Surface delete failures (e.g. permission denied) instead of silently
        // leaving the item in place; this status takes priority over refresh's.
        if let Some(err) = last_err {
            self.set_status(format!("delete failed for {} item(s): {}", failed, err));
        }
    }

    fn cancel_integration_install_prompt(&mut self) {
        self.confirm_integration_install_button_focus = 1;
        self.mode = AppMode::Integrations;
        self.clear_integration_install_prompt();
        self.set_status("integration install cancelled");
    }

    fn handle_ok_cancel_focus_key(key: KeyCode, focus: &mut u8, allow_hl_tab: bool) -> bool {
        match key {
            KeyCode::Left => {
                *focus = 0;
                true
            }
            KeyCode::Right => {
                *focus = 1;
                true
            }
            KeyCode::Char('h') if allow_hl_tab => {
                *focus = 0;
                true
            }
            KeyCode::Char('l') | KeyCode::Tab if allow_hl_tab => {
                *focus = 1;
                true
            }
            _ => false,
        }
    }

    fn handle_confirm_integration_install_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        if Self::handle_ok_cancel_focus_key(
            key.code,
            &mut self.confirm_integration_install_button_focus,
            true,
        ) {
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('y') => {
                self.confirm_integration_install_button_focus = 0;
                self.confirm_integration_install()?;
                Ok(true)
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.cancel_integration_install_prompt();
                Ok(false)
            }
            KeyCode::Enter => {
                if self.confirm_integration_install_button_focus == 0 {
                    self.confirm_integration_install()?;
                    Ok(true)
                } else {
                    self.cancel_integration_install_prompt();
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    fn handle_confirm_delete_key(&mut self, key: KeyEvent) {
        if Self::handle_ok_cancel_focus_key(key.code, &mut self.confirm_delete_button_focus, false)
        {
            return;
        }

        match key.code {
            KeyCode::Up => {
                self.confirm_delete_scroll_offset = self.confirm_delete_scroll_offset.saturating_sub(1);
            }
            KeyCode::Down => {
                self.confirm_delete_scroll_offset =
                    (self.confirm_delete_scroll_offset + 1).min(self.confirm_delete_max_offset);
            }
            KeyCode::PageUp => {
                self.confirm_delete_scroll_offset = self.confirm_delete_scroll_offset.saturating_sub(8);
            }
            KeyCode::PageDown => {
                self.confirm_delete_scroll_offset =
                    (self.confirm_delete_scroll_offset + 8).min(self.confirm_delete_max_offset);
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                if key.code == KeyCode::Enter && self.confirm_delete_button_focus == 1 {
                    self.mode = AppMode::Browsing;
                } else {
                    self.confirm_delete_selected_targets();
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = AppMode::Browsing;
            }
            _ => {}
        }
    }

    fn handle_confirm_extract_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') => {
                self.mode = AppMode::Browsing;
                self.extract_archives_confirmed();
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.archive_extract_targets.clear();
                self.mode = AppMode::Browsing;
                self.set_status("extract cancelled");
            }
            _ => {}
        }
    }









    fn panel_tab_bar_line(active: u8, theme_id: ui::theme::ThemeId, nerd_font: bool, avail_width: u16) -> Line<'static> {
        ui::panels::panel_tab_bar_line(active, theme_id, nerd_font, avail_width)
    }

    fn dual_panel_frame_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
        if !self.is_dual_panel_mode() || self.mode != AppMode::Browsing {
            return None;
        }

        let footer_height = 1;
        let header_reserved_rows = 1;
        let chunks = Layout::default()
            .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
            .split(area);

        let content_area = Rect::new(
            chunks[0].x,
            chunks[0].y + header_reserved_rows,
            chunks[0].width,
            chunks[0].height.saturating_sub(header_reserved_rows),
        );

        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_area);
        Some((split[0], split[1]))
    }

    fn update_list_double_click_state(
        last_click: &mut Option<(PathBuf, usize, Instant)>,
        current_dir: &PathBuf,
        target_idx: usize,
    ) -> bool {
        let now = Instant::now();
        let is_double_click = last_click
            .as_ref()
            .map(|(last_dir, last_idx, last_ts)| {
                *last_idx == target_idx
                    && *last_dir == *current_dir
                    && now.duration_since(*last_ts)
                        <= Duration::from_millis(MAIN_LIST_DOUBLE_CLICK_WINDOW_MS)
            })
            .unwrap_or(false);

        *last_click = if is_double_click {
            None
        } else {
            Some((current_dir.clone(), target_idx, now))
        };

        is_double_click
    }

    fn scrollbar_grab_offset_for_row(
        sb_area: Rect,
        total_rows: usize,
        offset: usize,
        row: u16,
    ) -> Option<u16> {
        let track_h = sb_area.height as usize;
        let visible_rows = sb_area.height.max(1) as usize;
        let max_scroll = total_rows.saturating_sub(visible_rows);
        if track_h == 0 || max_scroll == 0 {
            return None;
        }

        let offset = offset.min(max_scroll);
        let thumb_h = ((visible_rows * track_h + total_rows.saturating_sub(1)) / total_rows)
            .max(1)
            .min(track_h);
        let scroll_space = track_h.saturating_sub(thumb_h);
        let thumb_y = if max_scroll == 0 {
            0
        } else {
            (offset * scroll_space + (max_scroll / 2)) / max_scroll
        };

        let row_rel = row.saturating_sub(sb_area.y) as usize;
        let in_thumb = row_rel >= thumb_y && row_rel < thumb_y + thumb_h;
        Some(if in_thumb {
            (row_rel.saturating_sub(thumb_y)) as u16
        } else {
            (thumb_h / 2) as u16
        })
    }

    fn load_bookmarks() -> Vec<(usize, Option<PathBuf>)> {
        let cfg = crate::util::config::SbPersistConfig::load();
        (0..=9).map(|i| {
            // Tombstone: config explicitly marks this slot deleted — overrides env var
            if cfg.bookmarks.get(&(i as u8)).map(|v| v == "<deleted>").unwrap_or(false) {
                return (i, None);
            }
            let path = env::var(format!("SB_BOOKMARK_{}", i))
                .ok()
                .or_else(|| cfg.bookmarks.get(&(i as u8)).cloned())
                .map(PathBuf::from)
                .filter(|p| p.is_dir());
            (i, path)
        }).collect()
    }

    fn delete_bookmark(&self, idx: usize) {
        let from_env = env::var(format!("SB_BOOKMARK_{}", idx)).is_ok();
        let mut cfg = crate::util::config::SbPersistConfig::load();
        if from_env {
            cfg.bookmarks.insert(idx as u8, "<deleted>".to_string());
        } else {
            cfg.bookmarks.remove(&(idx as u8));
        }
        let _ = cfg.save();
    }

    fn handle_confirm_delete_bookmark_key(&mut self, key: KeyEvent) {
        if Self::handle_ok_cancel_focus_key(key.code, &mut self.confirm_delete_bookmark_button_focus, false) {
            return;
        }
        match key.code {
            KeyCode::Char('y') => {
                self.confirm_delete_bookmark_button_focus = 0;
                self.delete_bookmark(self.bookmark_delete_idx);
                self.mode = AppMode::Bookmarks;
            }
            KeyCode::Enter => {
                if self.confirm_delete_bookmark_button_focus == 0 {
                    self.delete_bookmark(self.bookmark_delete_idx);
                }
                self.mode = AppMode::Bookmarks;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = AppMode::Bookmarks;
            }
            _ => {}
        }
    }

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
    app.cleanup_archive_mounts();
    app.cleanup_ssh_mounts();
    let mut persist = util::config::SbPersistConfig::load();
    persist.view_mode = format!("{:?}", app.view_mode);
    persist.current_theme = ui::theme::theme_name(app.active_theme).to_string();
    persist.disabled_integrations = app
        .integration_overrides
        .iter()
        .filter_map(|(k, &v)| if !v { Some(k.clone()) } else { None })
        .collect();
    let _ = persist.save();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        TermClear(ClearType::All),
        MoveTo(0, 0)
    )?;
    let _ = std::fs::write("/tmp/sb_path", app.active_panel_dir().to_string_lossy().as_bytes());
    Ok(())
}