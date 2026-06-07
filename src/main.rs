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
    cursor::{Hide, MoveTo, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
        KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
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
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod integration;
mod app_archive;
mod app_git;
mod app_images;
mod app_input;
mod app_files;
mod app_meta;
mod app_preview;
mod app_render_cache;
pub(crate) use app_render_cache::{EntryRenderCache, EntryRenderConfig};
mod app_model;
pub(crate) use app_model::*;
mod app_search;
mod app_sizes;
mod app_sqlite;
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
    os_icon: Option<(&'static str, ratatui::style::Color)>,
    no_color: bool,
    show_icons: bool,
    integration_selected: usize,
    bookmark_selected: usize,
    integration_overrides: HashMap<String, bool>,
    integration_rows_cache: Vec<IntegrationRow>,
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
                if let Ok(mut child) = Command::new("hexyl")
                    .arg(path)
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    if let Some(hex_out) = child.stdout.take() {
                        let _ = Command::new("less").args(["-R"]).stdin(hex_out).status();
                    }
                    let _ = child.wait();
                }
                return Ok(());
            }
        }

        // For text files or if no binary editors available, use regular editor
        let editor = env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
        let _ = Command::new(editor).arg(path).status()?;
        Ok(())
    }

    fn new() -> io::Result<Self> {
        let current_dir = env::current_dir()?;
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
            os_icon: ui::icons::os_nerd_icon().map(|(g, _)| {
                (g, ui::theme::theme_spec(ui::theme::ThemeId::Original).icon_os)
            }),
            no_color: env_flag_true(&["NO_COLOR"]),
            show_icons: env::var("TERMINAL_ICONS").map(|v| v != "0").unwrap_or(true),
            integration_selected: 0,
            bookmark_selected: 0,
            integration_overrides: HashMap::new(),
            integration_rows_cache: Vec::new(),
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
            active_theme: ui::theme::ThemeId::Original,
            theme_selected: 0,
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
        app.set_active_theme(ui::theme::theme_by_name(&persist.current_theme));
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

    fn pump_preview_progress(&mut self) {
        let Some(rx) = self.preview_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb,
            }) => {
                if request_id == self.preview_request_id {
                    self.preview_target_path = Some(path.clone());
                    if image_rgb.is_none() {
                        self.preview_cache
                            .insert(path.clone(), (lines.clone(), line_kinds.clone(), footer.clone()));
                    }
                    self.preview_lines = lines;
                    self.preview_line_kinds = line_kinds;
                    self.preview_footer = footer;
                    if let Some((ref rgb, w, h)) = image_rgb {
                        self.preview_image_png = App::encode_rgb_to_png(rgb, w, h);
                    } else {
                        self.preview_image_png = None;
                    }
                    self.preview_image_rgb = image_rgb;
                    self.preview_pending = false;
                    self.preview_scroll_offset = 0;
                }
            }
            Ok(PreviewContentMsg::Failed {
                request_id,
                path,
                message,
            }) => {
                if request_id == self.preview_request_id {
                    self.preview_target_path = Some(path);
                    self.preview_lines = vec![message];
                    self.preview_line_kinds = vec![PreviewLineKind::Plain];
                    self.preview_footer = None;
                self.preview_image_rgb = None;
                self.preview_image_png = None;
                    self.preview_pending = false;
                    self.preview_scroll_offset = 0;
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.preview_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.preview_pending = false;
            }
        }
    }

    fn build_preview_content(
        request_id: u64,
        path: PathBuf,
        use_bat: bool,
        use_file: bool,
        use_resvg: bool,
        show_icons: bool,
        nerd_font_active: bool,
        theme_id: ui::theme::ThemeId,
    ) -> PreviewContentMsg {
        if path.is_dir() {
            let mut entries = Vec::new();
            let mut line_kinds = Vec::new();
            let mut names = Vec::new();
            if let Ok(read_dir) = fs::read_dir(&path) {
                for item in read_dir.flatten().take(500) {
                    names.push(item.path());
                }
            }
            names.sort_by(|a, b| {
                let a_name = a
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let b_name = b
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                a_name.cmp(&b_name)
            });

            for entry_path in names {
                let file_name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| entry_path.to_string_lossy().into_owned());
                let is_symlink = entry_path
                    .symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                let is_dir = entry_path.is_dir();
                #[cfg(unix)]
                let is_executable = {
                    use std::os::unix::fs::PermissionsExt;
                    !is_dir
                        && entry_path
                            .metadata()
                            .map(|m| m.permissions().mode() & 0o111 != 0)
                            .unwrap_or(false)
                };
                #[cfg(not(unix))]
                let is_executable = false;
                let is_hidden = file_name.starts_with('.');

                let (icon_glyph, icon_style) = App::icon_for_name(
                    &file_name,
                    is_dir,
                    show_icons,
                    nerd_font_active,
                    is_symlink,
                    theme_id,
                );
                let icon_prefix = if show_icons && !icon_glyph.is_empty() {
                    format!(" {} ", icon_glyph)
                } else {
                    String::new()
                };
                let suffix = if is_dir { "/" } else { "" };
                entries.push(format!("{}{}{}", icon_prefix, file_name, suffix));

                let mut style = if is_symlink {
                    Style::default().fg(ui::palette::Palette::SYMLINK)
                } else if is_dir {
                    Style::default()
                        .fg(ui::palette::Palette::ACCENT_PRIMARY)
                        .add_modifier(Modifier::BOLD)
                } else if is_executable {
                    Style::default().fg(ui::palette::Palette::SUCCESS_ALT)
                } else {
                    icon_style.fg.map_or_else(
                        || Style::default().fg(ui::palette::Palette::TEXT_NORMAL),
                        |fg| Style::default().fg(fg),
                    )
                };

                if is_hidden {
                    style = style.add_modifier(Modifier::DIM);
                }

                line_kinds.push(PreviewLineKind::Styled {
                    fg: style.fg,
                    bold: style.add_modifier.contains(Modifier::BOLD),
                    dim: style.add_modifier.contains(Modifier::DIM),
                });
            }

            if entries.is_empty() {
                entries.push("[empty folder]".to_string());
                line_kinds.push(PreviewLineKind::Plain);
            }

            let footer = App::compute_total_display_bytes(&path)
                .ok()
                .map(|bytes| format!("Total: {}", App::format_size(bytes)));
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines: entries,
                line_kinds,
                footer,
                image_rgb: None,
            };
        }

        if !path.exists() {
            return PreviewContentMsg::Failed {
                request_id,
                path,
                message: "[file not found]".to_string(),
            };
        }

        if App::is_svg_file(&path) && use_resvg {
            let image_rgb = App::decode_svg_to_rgb_scaled(&path);
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let lines = if image_rgb.is_none() {
                vec!["[svg could not be rendered]".to_string()]
            } else {
                Vec::new()
            };
            let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb,
            };
        }

        if App::is_image_file(&path) {
            let image_rgb = App::decode_image_to_rgb_scaled(&path);
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let lines = if image_rgb.is_none() {
                vec!["[image could not be decoded]".to_string()]
            } else {
                Vec::new()
            };
            let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb,
            };
        }

        let mut lines: Vec<String> = Vec::new();

        if App::is_binary_file(&path) {
            lines.push("[binary file]".to_string());
            if use_file {
                if let Ok(out) = Command::new("file").arg("-b").arg(&path).output() {
                    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !text.is_empty() {
                        lines.push(text);
                    }
                }
            }
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                line_kinds,
                footer,
                image_rgb: None,
            };
        }

        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > 10 * 1024 * 1024 {
            lines.push("[preview truncated: file larger than 10MB]".to_string());
        }

        // Number of leading header lines (e.g. truncation notice) that should
        // not receive a line-number gutter.
        let header_len = lines.len();

        if use_bat {
            if let Ok(out) = Command::new("bat")
                .args(["--paging=never", "--style=numbers", "--color=always", "--line-range", "1:220"])
                .arg(&path)
                .output()
            {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).into_owned();
                    lines.extend(text.lines().take(220).map(|s| s.to_string()));
                }
            }
        }

        // Fall back to reading the file directly when bat is unavailable or
        // produced no content, prepending our own line-number gutter.
        if lines.len() == header_len {
            let mut bytes = Vec::new();
            if let Ok(mut file) = fs::File::open(&path) {
                let _ = file.read_to_end(&mut bytes);
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            lines.extend(
                text.lines()
                    .take(220)
                    .enumerate()
                    .map(|(idx, line)| format!("\x1b[38;5;240m{:>4} │\x1b[0m {}", idx + 1, line)),
            );
        }

        if lines.is_empty() {
            lines.push("[no preview output]".to_string());
        }

        let footer = Some(format!("Size: {}", App::format_size(size)));
        let line_kinds = vec![PreviewLineKind::Plain; lines.len()];
        PreviewContentMsg::Ready {
            request_id,
            path,
            lines,
            line_kinds,
            footer,
            image_rgb: None,
        }
    }

    fn preview_json_with_jnv(path: &PathBuf) -> io::Result<bool> {
        let mut child = Command::new("jnv").arg(path).spawn();
        if let Ok(ref mut proc) = child {
            println!("Viewing JSON: {}", path.display());
            println!("Press q, Esc, or Left to close preview.");
            enable_raw_mode()?;
            loop {
                if proc.try_wait()?.is_some() {
                    break;
                }
                if event::poll(Duration::from_millis(120))? {
                    if let Event::Key(k) = event::read()? {
                        if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left) {
                            let _ = proc.kill();
                            let _ = proc.wait();
                            break;
                        }
                    }
                }
            }
            disable_raw_mode()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn preview_single_image_with_tool(path: &PathBuf, tool: &str) -> bool {
        let script = r#"
tool="$1"
img="$2"
clear
"$tool" -- "$img"
printf '\n[Press any key to return]\n'
IFS= read -rsn1 _
"#;

        Command::new("bash")
            .arg("-lc")
            .arg(script)
            .arg("--")
            .arg(tool)
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn preview_cast_with_asciinema(path: &PathBuf) -> io::Result<bool> {
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;

        let mut child = match Command::new("asciinema")
            .arg("play")
            .arg(path)
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        println!("Playing cast: {}", path.display());
        println!("Press q or Esc to stop playback.");

        enable_raw_mode()?;
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if event::poll(Duration::from_millis(120))? {
                if let Event::Key(k) = event::read()? {
                    if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
        disable_raw_mode()?;
        Ok(true)
    }


    fn sort_mode_options() -> [SortMode; 7] {
        [
            SortMode::NameAsc,
            SortMode::NameDesc,
            SortMode::ExtensionAsc,
            SortMode::SizeAsc,
            SortMode::SizeDesc,
            SortMode::ModifiedNewest,
            SortMode::ModifiedOldest,
        ]
    }

    fn sort_mode_index(mode: SortMode) -> usize {
        Self::sort_mode_options()
            .iter()
            .position(|m| *m == mode)
            .unwrap_or(0)
    }

    fn entry_name_key(entry: &fs::DirEntry) -> String {
        entry.file_name().to_string_lossy().to_ascii_lowercase()
    }

    fn entry_extension_key(entry: &fs::DirEntry) -> String {
        entry.path()
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    }

    pub(crate) fn sort_entries_by_mode(
        entries: &mut Vec<fs::DirEntry>,
        mode: SortMode,
        folder_size_cache: Option<&HashMap<PathBuf, u64>>,
    ) {
        if entries.len() < 2 {
            return;
        }
        // Pre-collect all sort keys in O(n) — eliminates O(n log n) stat() calls that
        // the previous sort_by comparator incurred by calling is_file()/metadata() per pair.
        let metas: Vec<Option<fs::Metadata>> = entries.iter().map(|e| e.metadata().ok()).collect();
        let is_dirs: Vec<bool> = metas.iter()
            .map(|m| m.as_ref().map(|m| m.is_dir()).unwrap_or(false))
            .collect();
        let names: Vec<String> = entries.iter().map(|e| Self::entry_name_key(e)).collect();
        let paths: Vec<PathBuf> = entries.iter().map(|e| e.path()).collect();
        let sizes: Vec<u64>    = metas.iter()
            .enumerate()
            .map(|(idx, m)| {
                let default_size = m.as_ref().map(|m| m.len()).unwrap_or(0);
                if !matches!(mode, SortMode::SizeAsc | SortMode::SizeDesc) {
                    return default_size;
                }

                if is_dirs[idx] {
                    folder_size_cache
                        .and_then(|cache| cache.get(&paths[idx]).copied())
                        .unwrap_or(0)
                } else {
                    default_size
                }
            })
            .collect();
        let times: Vec<u64>    = metas.iter().map(|m| {
            m.as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0)
        }).collect();
        let exts: Vec<String>  = entries.iter().map(|e| Self::entry_extension_key(e)).collect();

        let mut indices: Vec<usize> = (0..entries.len()).collect();
        indices.sort_by(|&a, &b| {
            // Directories always sort before files.
            let type_ord = is_dirs[b].cmp(&is_dirs[a]);
            if type_ord != std::cmp::Ordering::Equal {
                return type_ord;
            }
            match mode {
                SortMode::NameAsc        => names[a].cmp(&names[b]),
                SortMode::NameDesc       => names[b].cmp(&names[a]),
                SortMode::ExtensionAsc   => exts[a].cmp(&exts[b]).then_with(|| names[a].cmp(&names[b])),
                SortMode::SizeAsc        => sizes[a].cmp(&sizes[b]).then_with(|| names[a].cmp(&names[b])),
                SortMode::SizeDesc       => sizes[b].cmp(&sizes[a]).then_with(|| names[a].cmp(&names[b])),
                SortMode::ModifiedNewest => times[b].cmp(&times[a]).then_with(|| names[a].cmp(&names[b])),
                SortMode::ModifiedOldest => times[a].cmp(&times[b]).then_with(|| names[a].cmp(&names[b])),
            }
        });

        // Rearrange entries in-place to match the sorted index permutation.
        let mut tmp: Vec<Option<fs::DirEntry>> = entries.drain(..).map(Some).collect();
        *entries = indices.into_iter().map(|i| tmp[i].take().unwrap()).collect();
    }

    fn apply_sort_to_current_entries(&mut self) {
        if !self.tree_expansion_levels.is_empty() {
            let selected_path = self.entries.get(self.selected_index).map(|e| e.path());
            let _ = self.refresh_entries();
            if let Some(path) = selected_path {
                if let Some(idx) = self.entries.iter().position(|e| e.path() == path) {
                    self.selected_index = idx;
                    self.table_state.select(Some(idx));
                }
            }
            return;
        }
        let selected_path = self.entries.get(self.selected_index).map(|e| e.path());
        let marked_paths: HashSet<PathBuf> = self
            .marked_indices
            .iter()
            .filter_map(|idx| self.entries.get(*idx).map(|e| e.path()))
            .collect();

        let folder_size_cache = if self.folder_size_enabled {
            Some(&self.folder_size_cache)
        } else {
            None
        };
        Self::sort_entries_by_mode(&mut self.entries, self.sort_mode, folder_size_cache);

        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
        };
        let uid_cache = App::build_uid_cache(&self.entries);
        let gid_cache = App::build_gid_cache(&self.entries);
            self.entry_render_cache = self.entries.iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        self.apply_cached_folder_size_columns();
        self.refresh_meta_identity_widths();

        self.marked_indices = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| marked_paths.contains(&entry.path()))
            .map(|(idx, _)| idx)
            .collect();

        if self.entries.is_empty() {
            self.selected_index = 0;
            self.table_state.select(None);
            return;
        }

        self.selected_index = selected_path
            .and_then(|p| self.entries.iter().position(|e| e.path() == p))
            .unwrap_or_else(|| self.selected_index.min(self.entries.len() - 1));
        self.table_state.select(Some(self.selected_index));
    }

    fn begin_sort_menu(&mut self) {
        self.panel_tab = 4;
        self.sort_menu_selected = Self::sort_mode_index(self.sort_mode);
        self.mode = AppMode::SortMenu;
    }

    fn set_active_theme(&mut self, theme_id: ui::theme::ThemeId) {
        self.active_theme = theme_id;
        self.theme_selected = ui::theme::THEMES
            .iter()
            .position(|theme| theme.id == theme_id)
            .unwrap_or(0);
        self.os_icon = ui::icons::os_nerd_icon().map(|(glyph, _)| {
            (glyph, ui::theme::theme_spec(theme_id).icon_os)
        });
        self.rebuild_render_caches();
    }

    fn rebuild_render_caches(&mut self) {
        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
        };
        let uid_cache = App::build_uid_cache(&self.entries);
        let gid_cache = App::build_gid_cache(&self.entries);
        self.entry_render_cache = self.entries
            .iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        if !self.right.entries.is_empty() {
            let uid_cache_r = App::build_uid_cache(&self.right.entries);
            let gid_cache_r = App::build_gid_cache(&self.right.entries);
            self.right.entry_render_cache = self.right.entries
                .iter()
                .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache_r, &gid_cache_r))
                .collect();
        }
    }

    fn apply_selected_theme(&mut self) {
        if let Some(theme) = ui::theme::THEMES.get(self.theme_selected) {
            self.set_active_theme(theme.id);
        }
    }

    fn commit_sort_menu_choice(&mut self) {
        let options = Self::sort_mode_options();
        if let Some(mode) = options.get(self.sort_menu_selected).copied() {
            self.sort_mode = mode;
            self.apply_sort_to_current_entries();
            self.set_status(format!("sort: {}", mode.label()));
        }
        self.mode = AppMode::Browsing;
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right_status_message = msg;
        } else {
            self.status_message = msg;
        }
    }

    fn panel_status_message(&self, side: DualPanelSide) -> Option<&str> {
        let msg = match side {
            DualPanelSide::Left => self.status_message.as_str(),
            DualPanelSide::Right => self.right_status_message.as_str(),
        };

        if msg.is_empty() {
            None
        } else {
            Some(msg)
        }
    }

    fn decorate_footer_message(&self, msg: &str) -> String {
        ui::status::decorate_footer_message(msg, self.nerd_font_active)
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
            self.right.dir = target;
            if self.refresh_right_panel_entries().is_err() {
                self.set_status("refresh failed");
            }
        } else {
            self.try_enter_dir(target);
        }
    }




    fn create_temp_selection_path(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        env::temp_dir().join(format!("{}_{}_{}.txt", prefix, std::process::id(), stamp))
    }

    fn parse_ssh_config() -> Vec<SshHost> {
        let config_path = match env::var("HOME") {
            Ok(h) => PathBuf::from(h).join(".ssh/config"),
            Err(_) => return Vec::new(),
        };
        let content = match fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let mut hosts: Vec<SshHost> = Vec::new();
        let mut current: Option<SshHost> = None;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let sep = trimmed.find(|c: char| c.is_ascii_whitespace() || c == '=');
            let (raw_key, raw_val) = match sep {
                Some(pos) => (&trimmed[..pos], trimmed[pos + 1..].trim_start_matches(|c: char| c == '=' || c.is_ascii_whitespace())),
                None => (trimmed, ""),
            };
            let key = raw_key.to_lowercase();
            let val = raw_val.to_string();
            if key == "host" || key == "match" {
                if let Some(h) = current.take() {
                    if !h.alias.contains('*') && !h.alias.contains('?') {
                        hosts.push(h);
                    }
                }
                if key == "host" {
                    if let Some(alias) = val.split_whitespace().find(|s| !s.contains('*') && !s.contains('?')).map(|s| s.to_string()) {
                        current = Some(SshHost { hostname: alias.clone(), alias, user: None, port: None, identity_file: None });
                    }
                }
            } else if let Some(ref mut h) = current {
                match key.as_str() {
                    "hostname" => h.hostname = val,
                    "user" => h.user = Some(val),
                    "port" => h.port = val.parse().ok(),
                    "identityfile" => h.identity_file = Some(val),
                    _ => {}
                }
            }
        }
        if let Some(h) = current {
            if !h.alias.contains('*') && !h.alias.contains('?') {
                hosts.push(h);
            }
        }
        hosts
    }

    fn parse_rclone_remotes() -> Vec<RemoteEntry> {
        let out = match Command::new("rclone").args(["listremotes", "--long"]).output() {
            Ok(o) if o.status.success() => o,
            _ => return Vec::new(),
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                // format: "name:   type"
                let mut parts = line.splitn(2, ':');
                let name = parts.next()?.trim().to_string();
                let rtype = parts.next().unwrap_or("").trim().to_string();
                if name.is_empty() { return None; }
                Some(RemoteEntry::Rclone { name, rtype })
            })
            .collect()
    }

    fn parse_local_mount_dirs() -> Vec<RemoteEntry> {
        let user = env::var("USER").unwrap_or_default();
        let uid = users::get_current_uid();
        let candidates: Vec<(&str, PathBuf)> = vec![
            ("media", PathBuf::from(format!("/media/{}", user))),
            ("run-media", PathBuf::from(format!("/run/media/{}", user))),
            ("mnt", PathBuf::from("/mnt")),
            ("gvfs", PathBuf::from(format!("/run/user/{}/gvfs", uid))),
        ];

        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut mounts: Vec<RemoteEntry> = Vec::new();

        for (source, root) in candidates {
            if !root.is_dir() {
                continue;
            }

            let entries = match fs::read_dir(&root) {
                Ok(rd) => rd,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() || !seen.insert(path.clone()) {
                    continue;
                }

                let child_name = entry.file_name().to_string_lossy().into_owned();
                let name = format!("{}:{}", source, child_name);
                mounts.push(RemoteEntry::LocalMount {
                    name,
                    mount_path: path,
                    source: source.to_string(),
                });
            }
        }

        mounts.sort_by(|a, b| a.alias().cmp(b.alias()));
        mounts
    }

    fn wait_for_mount_ready(path: &PathBuf) {
        // Some backends (notably rclone --daemon) return before the mount is fully ready.
        // Poll briefly so the first directory read after enter is accurate.
        for _ in 0..20 {
            let ready = Command::new("mountpoint")
                .args(["-q", path.to_string_lossy().as_ref()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ready {
                break;
            }
            thread::sleep(Duration::from_millis(120));
        }
    }

    fn refresh_remote_entries(&mut self) {
        let has_sshfs = self.integration_active("sshfs");
        let has_rclone = self.integration_active("rclone");
        let mut entries: Vec<RemoteEntry> = Vec::new();
        if has_sshfs {
            entries.extend(App::parse_ssh_config().into_iter().map(RemoteEntry::Ssh));
        }
        if has_rclone {
            entries.extend(App::parse_rclone_remotes());
        }
        entries.extend(self.archive_mounts.iter().map(|m| RemoteEntry::ArchiveMount {
            archive_name: m.archive_name.clone(),
            mount_path: m.mount_path.clone(),
        }));
        entries.extend(App::parse_local_mount_dirs());
        self.remote_entries = entries;
        if self.remote_entries.is_empty() {
            self.ssh_picker_selection = 0;
        } else {
            self.ssh_picker_selection = self.ssh_picker_selection.min(self.remote_entries.len() - 1);
        }
    }

    fn remote_mount_for_path(&self, path: &PathBuf) -> Option<&SshMount> {
        self.ssh_mounts
            .iter()
            .filter(|mount| path == &mount.mount_path || path.starts_with(&mount.mount_path))
            .max_by_key(|mount| mount.mount_path.components().count())
    }

    fn current_remote_mount(&self) -> Option<&SshMount> {
        self.remote_mount_for_path(&self.current_dir)
    }

    fn current_header_identity(&self, local_user: &str, local_host: &str) -> String {
        self.current_remote_mount()
            .map(|mount| mount.remote_label.clone())
            .unwrap_or_else(|| format!("{}@{}", local_user, local_host))
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

    fn current_dir_display_path(&self) -> String {
        self.display_path_for(&self.current_dir)
    }

    fn path_filter_suffix_text(&self) -> Option<String> {
        let filter = self.path_input_filter.as_ref()?;
        let suffix = match filter.mode {
            PathFilterMode::Prefix => format!("^{}", filter.pattern),
            PathFilterMode::Suffix => format!("{}$", filter.pattern),
            PathFilterMode::Contains => format!("~{}", filter.pattern),
        };
        Some(suffix)
    }

    fn path_with_filter_suffix(base: String, suffix: Option<String>) -> String {
        let Some(suffix) = suffix else {
            return base;
        };

        if base == "/" {
            format!("/{}", suffix)
        } else {
            format!("{}/{}", base, suffix)
        }
    }

    fn current_dir_display_path_with_filter(&self) -> String {
        Self::path_with_filter_suffix(self.current_dir_display_path(), self.path_filter_suffix_text())
    }

    fn current_path_edit_value(&self) -> String {
        let base = self.current_dir.to_string_lossy().into_owned();
        Self::path_with_filter_suffix(base, self.path_filter_suffix_text())
    }

    fn mount_rclone_remote(&mut self, name: &str, rtype: &str) -> io::Result<()> {
        let return_dir = self.active_panel_dir();
        // If already mounted, just navigate there
        if let Some(existing) = self.ssh_mounts.iter_mut().find(|m| m.host_alias == name) {
            existing.return_dir = return_dir.clone();
            let mount_path = existing.mount_path.clone();
            self.mode = AppMode::Browsing;
            self.try_enter_dir_on_active_panel(mount_path);
            return Ok(());
        }
        let _ = rtype; // informational only
        let mount_dir = PathBuf::from(format!("/tmp/sbrs_rclone_{}", name));
        if mount_dir.exists() {
            let _ = fs::remove_dir(&mount_dir);
        }
        fs::create_dir_all(&mount_dir)?;
        let remote_spec = format!("{}:", name);
        let status = Command::new("rclone")
            .args(["mount", &remote_spec, mount_dir.to_str().unwrap_or(""),
                   "--daemon", "--vfs-cache-mode", "writes"])
            .status()?;
        if status.success() {
            Self::wait_for_mount_ready(&mount_dir);
            let remote_os_icon = ui::icons::remote_os_nerd_icon(&mount_dir)
                .map(|(g, _)| (g, ui::theme::theme_spec(self.active_theme).icon_os));
            self.ssh_mounts.push(SshMount {
                host_alias: name.to_string(),
                mount_path: mount_dir.clone(),
                return_dir,
                remote_label: name.to_string(),
                remote_root: "/".to_string(),
                remote_os_icon,
            });
            self.mode = AppMode::Browsing;
            self.try_enter_dir_on_active_panel(mount_dir);
            Ok(())
        } else {
            let _ = fs::remove_dir(&mount_dir);
            Err(io::Error::new(io::ErrorKind::Other, "rclone mount failed"))
        }
    }

    fn detect_ssh_remote_os_icon(host: &SshHost, theme_id: ui::theme::ThemeId) -> Option<(&'static str, Color)> {
        let target = match &host.user {
            Some(u) => format!("{}@{}", u, host.hostname),
            None => host.hostname.clone(),
        };
        let mut cmd = Command::new("ssh");
        if let Some(port) = host.port {
            cmd.args(["-p", &port.to_string()]);
        }
        if let Some(idf) = &host.identity_file {
            let expanded = idf.replace('~', &env::var("HOME").unwrap_or_default());
            cmd.args(["-i", &expanded]);
        }
        let output = cmd.args([&target, "cat", "/etc/os-release"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let content = String::from_utf8_lossy(&output.stdout);
        ui::icons::os_nerd_icon_from_os_release_content(content.as_ref())
            .map(|(g, _)| (g, ui::theme::theme_spec(theme_id).icon_os))
    }

    fn mount_ssh_host(&mut self, host: &SshHost) -> io::Result<()> {
        let return_dir = self.active_panel_dir();
        // If already mounted, just navigate there
        if let Some(existing) = self.ssh_mounts.iter_mut().find(|m| m.host_alias == host.alias) {
            existing.return_dir = return_dir.clone();
            if existing.remote_os_icon.is_none() {
                existing.remote_os_icon = Self::detect_ssh_remote_os_icon(host, self.active_theme);
            }
            let mount_path = existing.mount_path.clone();
            self.mode = AppMode::Browsing;
            self.try_enter_dir_on_active_panel(mount_path);
            return Ok(());
        }
        let mount_dir = PathBuf::from(format!("/tmp/sbrs_sshfs_{}", host.alias));
        // Remove stale dir if it exists but isn't mounted
        if mount_dir.exists() {
            let _ = fs::remove_dir(&mount_dir);
        }
        fs::create_dir_all(&mount_dir)?;
        let remote_spec = match &host.user {
            Some(u) => format!("{}@{}:", u, host.hostname),
            None => format!("{}:", host.hostname),
        };
        let mut cmd = Command::new("sshfs");
        if let Some(port) = host.port {
            cmd.args(["-p", &port.to_string()]);
        }
        if let Some(idf) = &host.identity_file {
            let expanded = idf.replace('~', &env::var("HOME").unwrap_or_default());
            cmd.args(["-o", &format!("IdentityFile={}", expanded)]);
        }
        cmd.arg(&remote_spec).arg(&mount_dir);
        let status = cmd.status()?;
        if status.success() {
            Self::wait_for_mount_ready(&mount_dir);
            let remote_label = match &host.user {
                Some(user) => format!("{}@{}", user, host.hostname),
                None => host.hostname.clone(),
            };
            let remote_os_icon = ui::icons::remote_os_nerd_icon(&mount_dir)
                .map(|(g, _)| (g, ui::theme::theme_spec(self.active_theme).icon_os))
                .or_else(|| Self::detect_ssh_remote_os_icon(host, self.active_theme));
            self.ssh_mounts.push(SshMount {
                host_alias: host.alias.clone(),
                mount_path: mount_dir.clone(),
                return_dir,
                remote_label,
                remote_root: "~".to_string(),
                remote_os_icon,
            });
            self.mode = AppMode::Browsing;
            self.try_enter_dir_on_active_panel(mount_dir);
            Ok(())
        } else {
            let _ = fs::remove_dir(&mount_dir);
            Err(io::Error::new(io::ErrorKind::Other, "sshfs mount failed"))
        }
    }

    fn try_leave_ssh_mount(&mut self) -> bool {
        // Check if we are at the mount root (not just a subdir) — only intercept at the boundary
        let mount_idx = self.ssh_mounts.iter().rposition(|m| {
            self.current_dir == m.mount_path
        });
        let Some(idx) = mount_idx else { return false };
        self.remember_current_selection();
        let return_dir = self.ssh_mounts[idx].return_dir.clone();
        // Navigate back without unmounting — mount stays active, shown as mounted in S picker
        self.current_dir = return_dir;
        self.refresh_entries_or_status();
        true
    }

    fn cleanup_ssh_mounts(&mut self) {
        // If current_dir is inside any ssh mount, set it to the return dir first
        // so the shell cd integration lands on a local path
        for mount in self.ssh_mounts.iter() {
            if self.current_dir == mount.mount_path || self.current_dir.starts_with(&mount.mount_path) {
                self.current_dir = mount.return_dir.clone();
                break;
            }
        }
        while let Some(mount) = self.ssh_mounts.pop() {
            let path_str = mount.mount_path.to_string_lossy().to_string();
            // Try fusermount -u, then fusermount3 -u, then lazy -z variants, then umount
            let ok = Command::new("fusermount").args(["-u", &path_str]).status().map(|s| s.success()).unwrap_or(false)
                || Command::new("fusermount3").args(["-u", &path_str]).status().map(|s| s.success()).unwrap_or(false)
                || Command::new("fusermount").args(["-uz", &path_str]).status().map(|s| s.success()).unwrap_or(false)
                || Command::new("fusermount3").args(["-uz", &path_str]).status().map(|s| s.success()).unwrap_or(false)
                || Command::new("umount").args([&path_str]).status().map(|s| s.success()).unwrap_or(false)
                || Command::new("umount").args(["-l", &path_str]).status().map(|s| s.success()).unwrap_or(false);
            let _ = ok; // best-effort; proceed regardless
            let _ = fs::remove_dir(&mount.mount_path);
        }
    }

    fn unmount_ssh_mount_by_alias(&mut self, alias: &str) -> bool {
        let Some(idx) = self.ssh_mounts.iter().rposition(|m| m.host_alias == alias) else {
            return false;
        };

        let mount = self.ssh_mounts.remove(idx);
        if self.current_dir == mount.mount_path || self.current_dir.starts_with(&mount.mount_path) {
            self.current_dir = mount.return_dir.clone();
            self.refresh_entries_or_status();
        }

        let path_str = mount.mount_path.to_string_lossy().to_string();
        let _ = Command::new("fusermount").args(["-u", &path_str]).status();
        let _ = Command::new("fusermount3").args(["-u", &path_str]).status();
        let _ = Command::new("fusermount").args(["-uz", &path_str]).status();
        let _ = Command::new("fusermount3").args(["-uz", &path_str]).status();
        let _ = Command::new("umount").args([&path_str]).status();
        let _ = Command::new("umount").args(["-l", &path_str]).status();
        let _ = fs::remove_dir(&mount.mount_path);
        true
    }

    fn open_ssh_shell_session(&mut self, host: &SshHost) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        execute!(io::stdout(), Show)?;

        // Match normal terminal behavior exactly: rely on OpenSSH host alias resolution
        // and config processing instead of overriding with parsed options.
        let mut cmd = Command::new("ssh");
        cmd.arg(&host.alias);

        let status = cmd.status();

        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        enable_raw_mode()?;
        execute!(io::stdout(), Hide)?;

        match status {
            Ok(exit_status) => {
                if exit_status.success() {
                    self.set_status(format!("SSH session closed: {}", host.alias));
                } else if let Some(code) = exit_status.code() {
                    self.set_status(format!("ssh exited with code {} for {}", code, host.alias));
                } else {
                    self.set_status(format!("ssh session ended for {}", host.alias));
                }
            }
            Err(e) => {
                self.set_status(format!("failed to start ssh session for {}: {}", host.alias, e));
            }
        }

        self.refresh_entries_or_status();
        Ok(())
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
        if let Some(rest) = trimmed.strip_prefix("~/") {
            if let Ok(home) = env::var("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
        if trimmed == "~" {
            if let Ok(home) = env::var("HOME") {
                return PathBuf::from(home);
            }
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
            if let Some(name) = last_created {
                if let Some(index) = self
                    .right.entries
                    .iter()
                    .position(|entry| entry.file_name().to_string_lossy() == name)
                {
                    self.right.selected_index = index;
                    self.right.table_state.select(Some(index));
                }
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

    fn begin_download_input(&mut self) {
        if self.download_rx.is_some() {
            self.set_status("download already in progress");
            return;
        }

        self.download_pending_url = None;
        self.download_pending_name = None;
        self.download_resume_input = None;
        self.begin_input_edit(AppMode::DownloadInput, String::new());
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

    fn parse_download_input(raw: &str) -> Result<(String, Option<String>), String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("enter a URL to download".to_string());
        }

        let (url, file_name) = if let Some(rest) = trimmed.strip_prefix('"') {
            let Some(end_quote) = rest.find('"') else {
                return Err("quoted URL is missing a closing quote".to_string());
            };
            let url = rest[..end_quote].trim().to_string();
            let remainder = rest[end_quote + 1..].trim();
            let file_name = if remainder.is_empty() {
                None
            } else {
                Some(remainder.to_string())
            };
            (url, file_name)
        } else if let Some(split_at) = trimmed.find(char::is_whitespace) {
            let url = trimmed[..split_at].trim().to_string();
            let remainder = trimmed[split_at..].trim();
            let file_name = if remainder.is_empty() {
                None
            } else {
                Some(remainder.to_string())
            };
            (url, file_name)
        } else {
            (trimmed.to_string(), None)
        };

        if url.is_empty() {
            return Err("enter a URL to download".to_string());
        }
        if !url.contains("://") {
            return Err("URL must include a scheme like https://".to_string());
        }

        Ok((url, file_name))
    }

    fn download_url_host(url: &str) -> Option<String> {
        let authority_and_path = url.split_once("://")?.1;
        let authority = authority_and_path
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default();
        let host_port = authority.rsplit('@').next().unwrap_or(authority);
        let host = if let Some(rest) = host_port.strip_prefix('[') {
            rest.split(']').next().unwrap_or_default().trim().to_string()
        } else {
            host_port.split(':').next().unwrap_or_default().trim().to_string()
        };

        if host.is_empty() {
            None
        } else {
            Some(host)
        }
    }

    fn download_url_file_name(url: &str) -> Option<String> {
        let authority_and_path = url.split_once("://")?.1;
        let path_and_more = authority_and_path.split_once('/').map(|(_, tail)| tail)?;
        let path = path_and_more
            .split(['?', '#'])
            .next()
            .unwrap_or_default();
        let name = path.rsplit('/').find(|segment| !segment.is_empty())?;
        if name == "." || name == ".." {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn validate_download_file_name(name: &str) -> Result<String, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("download name cannot be empty".to_string());
        }
        if trimmed == "." || trimmed == ".." {
            return Err("download name cannot be . or ..".to_string());
        }
        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err("download name cannot contain path separators".to_string());
        }
        Ok(trimmed.to_string())
    }

    fn queue_download_request(&mut self, url: String, file_name: String, resume_input: String) {
        self.download_pending_url = Some(url.clone());
        self.download_pending_name = Some(file_name.clone());
        self.download_resume_input = Some(resume_input);

        if self.current_dir.join(&file_name).exists() {
            self.clear_input_edit();
            self.mode = AppMode::ConfirmDownloadOverwrite;
            self.set_status(format!("target exists: overwrite {}?", file_name));
            return;
        }

        self.start_download_job(url, file_name);
    }

    fn submit_download_input(&mut self) {
        let resume_input = self.input_buffer.trim().to_string();
        let (url, explicit_name) = match Self::parse_download_input(&resume_input) {
            Ok(parsed) => parsed,
            Err(message) => {
                self.set_status(message);
                return;
            }
        };

        if let Some(name) = explicit_name {
            match Self::validate_download_file_name(&name) {
                Ok(file_name) => self.queue_download_request(url, file_name, resume_input),
                Err(message) => self.set_status(message),
            }
            return;
        }

        if let Some(name) = Self::download_url_file_name(&url) {
            match Self::validate_download_file_name(&name) {
                Ok(file_name) => self.queue_download_request(url, file_name, resume_input),
                Err(message) => self.set_status(message),
            }
            return;
        }

        let Some(host_name) = Self::download_url_host(&url) else {
            self.set_status("could not derive a file name from URL");
            return;
        };

        self.download_pending_url = Some(url);
        self.download_pending_name = None;
        self.download_resume_input = Some(resume_input);
        self.begin_input_edit(AppMode::DownloadNaming, host_name);
        self.set_status("edit download name and press Enter");
    }

    fn submit_download_name(&mut self) {
        let Some(url) = self.download_pending_url.clone() else {
            self.mode = AppMode::Browsing;
            self.clear_input_edit();
            self.set_status("download target is missing");
            return;
        };

        match Self::validate_download_file_name(&self.input_buffer) {
            Ok(file_name) => {
                let resume_input = format!("\"{}\" {}", url, file_name);
                self.queue_download_request(url, file_name, resume_input);
            }
            Err(message) => self.set_status(message),
        }
    }

    fn cancel_download_overwrite(&mut self) {
        let resume_input = self.download_resume_input.clone().unwrap_or_default();
        self.begin_input_edit(AppMode::DownloadInput, resume_input);
        self.download_pending_name = None;
        self.set_status("download overwrite cancelled");
    }

    fn preferred_download_tool(&self) -> Option<&'static str> {
        if self.integration_active("wget") {
            Some("wget")
        } else if self.integration_active("curl") {
            Some("curl")
        } else {
            None
        }
    }

    fn start_download_job(&mut self, url: String, file_name: String) {
        if self.download_rx.is_some() {
            self.set_status("download already in progress");
            return;
        }

        let Some(tool) = self.preferred_download_tool() else {
            self.set_status("wget/curl not found in PATH");
            return;
        };

        let output_path = self.current_dir.join(&file_name);
        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        self.download_active_name = file_name.clone();
        self.download_pending_url = None;
        self.download_pending_name = None;
        self.download_resume_input = None;
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
        self.set_status(format!("downloading {} via {}", file_name, tool));

        thread::spawn(move || {
            let result = util::command::CommandBuilder::download_with_progress(tool, &url, &output_path, |hint| {
                let _ = tx.send(DownloadProgressMsg::Status(hint.to_string()));
            });

            let _ = tx.send(DownloadProgressMsg::Finished { file_name, result });
        });
    }

    /// Drains all pending download messages for this frame.
    ///
    /// `Disconnected` only means the sender is gone **and** the queue is empty; any `Finished`
    /// message was already delivered as `Ok(Finished)` in this same drain loop (never skip the
    /// `finished` block by returning early on `Disconnected`).
    fn pump_download_progress(&mut self) {
        let Some(rx) = self.download_rx.take() else {
            return;
        };

        let mut finished: Option<(String, Result<(), String>)> = None;
        let mut latest_status: Option<String> = None;
        let mut channel_closed = false;
        loop {
            match rx.try_recv() {
                Ok(DownloadProgressMsg::Status(s)) => latest_status = Some(s),
                Ok(DownloadProgressMsg::Finished { file_name, result }) => {
                    finished = Some((file_name, result));
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Queue empty and all senders dropped. If the worker exited normally, we already
                    // received `Ok(Finished)` above; otherwise `finished` stays empty.
                    channel_closed = true;
                    break;
                }
            }
        }

        if let Some((file_name, result)) = finished {
            self.download_rx = None;
            self.download_active_name.clear();
            self.refresh_entries_or_status();
            match result {
                Ok(()) => {
                    self.select_entry_named(&file_name);
                    self.set_status(format!("downloaded {}", file_name));
                }
                Err(error) => {
                    self.set_status(format!("download failed for {}: {}", file_name, error));
                }
            }
            return;
        }

        if channel_closed {
            self.download_active_name.clear();
            self.download_rx = None;
            self.set_status("download worker disconnected");
            return;
        }

        if let Some(s) = latest_status {
            let name = self.download_active_name.clone();
            if name.is_empty() {
                self.set_status(format!("downloading: {}", s));
            } else {
                self.set_status(format!("downloading {} — {}", name, s));
            }
        }

        self.download_rx = Some(rx);
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
                .filter(|e| self.show_hidden || !e.file_name().to_string_lossy().starts_with('.'))
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
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if Self::entry_name_matches_path_filter(&name, &filter_regex) {
                        filtered_entries.push(entry);
                        filtered_prefixes.push(prefix);
                    }
                }
                entries = filtered_entries;
                tree_row_prefixes = filtered_prefixes;
            } else {
                entries.retain(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
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

    fn notes_file_path(dir: &PathBuf) -> PathBuf {
        dir.join(".sb")
    }

    fn escape_note_field(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                _ => out.push(ch),
            }
        }
        out
    }

    fn unescape_note_field(input: &str) -> Option<String> {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                out.push(ch);
                continue;
            }

            let esc = chars.next()?;
            match esc {
                '\\' => out.push('\\'),
                't' => out.push('\t'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                _ => return None,
            }
        }
        Some(out)
    }

    fn load_notes_map_for_dir(dir: &PathBuf) -> HashMap<String, String> {
        let path = Self::notes_file_path(dir);
        let Ok(raw) = fs::read_to_string(path) else {
            return HashMap::new();
        };

        let mut notes = HashMap::new();
        for line in raw.lines() {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let Some(name_raw) = parts.next() else {
                continue;
            };
            let Some(note_raw) = parts.next() else {
                continue;
            };
            let Some(name) = Self::unescape_note_field(name_raw) else {
                continue;
            };
            let Some(note) = Self::unescape_note_field(note_raw) else {
                continue;
            };
            if name.is_empty() || note.trim().is_empty() {
                continue;
            }
            notes.insert(name, note);
        }
        notes
    }

    fn request_notes_for_current_dir_once(&mut self) {
        if self.notes_rx.is_some() {
            return;
        }
        if self
            .notes_loaded_for
            .as_ref()
            .map(|p| p == &self.current_dir)
            .unwrap_or(false)
        {
            return;
        }

        self.notes_scan_id = self.notes_scan_id.wrapping_add(1);
        let scan_id = self.notes_scan_id;
        let dir = self.current_dir.clone();
        self.notes_by_name.clear();
        let (tx, rx) = mpsc::channel();
        self.notes_rx = Some(rx);

        thread::spawn(move || {
            let notes = App::load_notes_map_for_dir(&dir);
            let _ = tx.send(NotesLoadMsg::Finished(scan_id, dir, notes));
        });
    }

    fn pump_notes_progress(&mut self) {
        let Some(rx) = self.notes_rx.take() else {
            return;
        };

        let mut keep_rx = true;
        loop {
            match rx.try_recv() {
                Ok(NotesLoadMsg::Finished(scan_id, path, notes)) => {
                    if scan_id == self.notes_scan_id && path == self.current_dir {
                        self.notes_by_name = notes;
                        self.notes_loaded_for = Some(path);
                        keep_rx = false;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    keep_rx = false;
                    break;
                }
            }
        }

        if keep_rx {
            self.notes_rx = Some(rx);
        }
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
        let (tx, rx) = mpsc::channel();
        self.right_notes_rx = Some(rx);
        thread::spawn(move || {
            let notes = App::load_notes_map_for_dir(&dir);
            let _ = tx.send(NotesLoadMsg::Finished(0, dir, notes));
        });
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

    fn selected_note_targets(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let is_right = self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right;
        if is_right {
            if !self.right.marked_indices.is_empty() {
                for idx in &self.right.marked_indices {
                    if let Some(entry) = self.right.entries.get(*idx) {
                        out.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            } else if let Some(entry) = self.right.entries.get(self.right.selected_index) {
                out.push(entry.file_name().to_string_lossy().into_owned());
            }
        } else {
            if !self.marked_indices.is_empty() {
                for idx in &self.marked_indices {
                    if let Some(entry) = self.entries.get(*idx) {
                        out.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            } else if let Some(entry) = self.entries.get(self.selected_index) {
                out.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn begin_note_edit(&mut self) {
        let targets = self.selected_note_targets();
        if targets.is_empty() {
            self.set_status("no selected item");
            return;
        }

        let active_dir = self.active_panel_dir();
        let notes_map = if active_dir != self.current_dir {
            Self::load_notes_map_for_dir(&active_dir)
        } else {
            self.notes_by_name.clone()
        };

        let initial = if targets.len() == 1 {
            notes_map.get(&targets[0]).cloned().unwrap_or_default()
        } else {
            String::new()
        };

        self.note_edit_dir = active_dir;
        self.note_edit_targets = targets;
        self.begin_input_edit(AppMode::NoteEditing, initial);
    }

    fn entry_names_in_dir(dir: &PathBuf) -> HashSet<String> {
        let mut names = HashSet::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return names;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".sb" {
                continue;
            }
            names.insert(name);
        }
        names
    }

    fn write_notes_map(dir: &PathBuf, notes: &HashMap<String, String>, scan_id: u64) -> io::Result<()> {
        let notes_path = Self::notes_file_path(dir);
        if notes.is_empty() {
            match fs::remove_file(&notes_path) {
                Ok(()) | Err(_) => {}
            }
            return Ok(());
        }
        let mut keys: Vec<&String> = notes.keys().collect();
        keys.sort();
        let mut lines: Vec<String> = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(note) = notes.get(key) {
                lines.push(format!(
                    "{}\t{}",
                    Self::escape_note_field(key),
                    Self::escape_note_field(note)
                ));
            }
        }
        let mut payload = lines.join("\n");
        payload.push('\n');
        let tmp_path = dir.join(format!(".sb.tmp.{}", scan_id));
        fs::write(&tmp_path, &payload)?;
        fs::rename(&tmp_path, &notes_path)?;
        Ok(())
    }

    fn save_notes_for_current_dir(&mut self) -> io::Result<()> {
        let existing = Self::entry_names_in_dir(&self.current_dir);
        self.notes_by_name
            .retain(|name, note| existing.contains(name) && !note.trim().is_empty());
        Self::write_notes_map(&self.current_dir, &self.notes_by_name, self.notes_scan_id)?;
        self.notes_loaded_for = Some(self.current_dir.clone());
        Ok(())
    }

    fn commit_note_edit(&mut self) {
        if self.note_edit_targets.is_empty() {
            self.clear_input_edit();
            self.mode = AppMode::Browsing;
            return;
        }

        let note = self.input_buffer.clone();
        let is_empty = note.trim().is_empty();
        let count = self.note_edit_targets.len();
        let edit_dir = self.note_edit_dir.clone();

        let save_result = if edit_dir == self.current_dir || edit_dir == PathBuf::new() {
            for target in &self.note_edit_targets {
                if is_empty {
                    self.notes_by_name.remove(target);
                } else {
                    self.notes_by_name.insert(target.clone(), note.clone());
                }
            }
            self.save_notes_for_current_dir()
        } else {
            let mut notes = Self::load_notes_map_for_dir(&edit_dir);
            let existing = Self::entry_names_in_dir(&edit_dir);
            notes.retain(|name, n| existing.contains(name) && !n.trim().is_empty());
            for target in &self.note_edit_targets {
                if is_empty {
                    notes.remove(target);
                } else {
                    notes.insert(target.clone(), note.clone());
                }
            }
            Self::write_notes_map(&edit_dir, &notes, self.notes_scan_id)
        };

        match save_result {
            Ok(()) => {
                if is_empty {
                    self.set_status(format!("cleared note for {} item(s)", count));
                } else {
                    self.set_status(format!("saved note for {} item(s)", count));
                }
            }
            Err(e) => {
                self.set_status(format!("save note failed: {}", e));
            }
        }

        self.note_edit_targets.clear();
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
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
        for path in to_delete {
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
        self.mode = AppMode::Browsing;
        self.refresh_entries_or_status();
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



    fn drop_to_shell(&mut self) -> io::Result<()> {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        execute!(io::stdout(), Show)?;
        let _ = Command::new(&shell)
            .current_dir(&self.current_dir)
            .status();
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        enable_raw_mode()?;
        execute!(io::stdout(), Hide)?;
        self.set_status("returned from shell");
        self.refresh_entries_or_status();
        Ok(())
    }

    fn open_path_in_view_mode(path: &PathBuf, use_pager: bool) -> io::Result<()> {
        if Self::is_image_file(path) {
            if Self::integration_probe("viu").0 {
                let _ = Command::new("viu").arg(path).status();
                return Ok(());
            }
            if Self::integration_probe("chafa").0 {
                let _ = Command::new("chafa").arg(path).status();
                return Ok(());
            }
        }

        if Self::is_markdown_file(path) && Self::integration_probe("glow").0 {
            let mut cmd = Command::new("glow");
            if use_pager {
                cmd.arg("-p");
            }
            let _ = cmd.arg(path).status();
            return Ok(());
        }

        if Self::is_mermaid_file(path) && Self::integration_probe("mmdflux").0 {
            if use_pager {
                if let Ok(mut child) = Command::new("mmdflux")
                    .arg(path)
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    if let Some(mmd_out) = child.stdout.take() {
                        let _ = Command::new("less").args(["-R"]).stdin(mmd_out).status();
                    }
                    let _ = child.wait();
                }
            } else {
                let _ = Command::new("mmdflux").arg(path).status();
            }
            return Ok(());
        }

        if Self::is_html_file(path) && Self::integration_probe("links").0 {
            let _ = Command::new("links").arg(path).status();
            return Ok(());
        }

        if Self::is_json_file(path) && Self::integration_probe("jnv").0 {
            let _ = Command::new("jnv").arg(path).status();
            return Ok(());
        }

        if Self::is_delimited_text_file(path) && Self::integration_probe("csvlens").0 {
            let _ = Command::new("csvlens").arg(path).status();
            return Ok(());
        }

        if Self::is_audio_file(path) && Self::integration_probe("sox").0 {
            if Self::integration_probe("play").0 {
                let _ = Command::new("play")
                    .arg(path)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            } else {
                let _ = Command::new("sox")
                    .arg(path)
                    .arg("-d")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            return Ok(());
        }

        if Self::is_pdf_file(path) && Self::integration_probe("pdftotext").0 {
            if use_pager {
                let mut shown = false;
                if let Ok(mut child) = Command::new("pdftotext")
                    .args(["-layout", "-nopgbrk"])
                    .arg(path)
                    .arg("-")
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    if let Some(pdf_text) = child.stdout.take() {
                        shown = Command::new("less")
                            .args(["-R"])
                            .stdin(pdf_text)
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false);
                    }
                    let _ = child.wait();
                }
                if !shown {
                    let _ = Command::new("less")
                        .args(["-R", path.to_str().unwrap_or_default()])
                        .status();
                }
            } else {
                let _ = Command::new("pdftotext")
                    .args(["-layout", "-nopgbrk"])
                    .arg(path)
                    .arg("-")
                    .status();
            }
            return Ok(());
        }

        if Self::is_cast_file(path) && Self::integration_probe("asciinema").0 {
            let _ = Command::new("asciinema").args(["play", "-i"]).arg(path).status();
            return Ok(());
        }

        if Self::is_binary_file(path) && Self::integration_probe("hexyl").0 {
            if use_pager {
                if let Ok(child) = Command::new("hexyl")
                    .arg(path)
                    .stdout(Stdio::piped())
                    .spawn()
                {
                    let _ = Command::new("less")
                        .args(["-R"])
                        .stdin(child.stdout.unwrap())
                        .status();
                    return Ok(());
                }
            } else {
                let _ = Command::new("hexyl").arg(path).status();
                return Ok(());
            }
        }

        if Self::integration_probe("bat").0 {
            let bat_cmd = Self::bat_tool().unwrap_or_else(|| "bat".to_string());
            let paging = if use_pager { "always" } else { "never" };
            let _ = Command::new(bat_cmd)
                .args([&format!("--paging={}", paging), "--style=full", "--color=always"])
                .arg(path)
                .status();
            return Ok(());
        }

        if use_pager {
            let _ = Command::new("less")
                .args(["-R", path.to_str().unwrap_or_default()])
                .status();
        } else {
            let _ = Command::new("cat")
                .arg(path)
                .status();
        }
        Ok(())
    }

    fn shell_single_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    fn open_split_shell_with_less(&mut self) -> io::Result<()> {
        if !self.integration_active("tmux") {
            self.set_status("tmux not found in PATH");
            return Ok(());
        }

        let Some(entry) = self.entries.get(self.selected_index) else {
            self.set_status("no selected item");
            return Ok(());
        };

        let selected_path = entry.path();
        if selected_path.is_dir() {
            self.set_status("split shell preview works on files only");
            return Ok(());
        }

        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let current_dir = self.current_dir.to_string_lossy().into_owned();
        let selected_file = selected_path.to_string_lossy().into_owned();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let session_name = format!("sbrs_i_{}_{}", std::process::id(), stamp % 1_000_000_000);

        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        execute!(io::stdout(), Show)?;

        let tmux_result = (|| -> io::Result<()> {
            let left_cmd = format!(
                "{} -i; tmux kill-session -t {} >/dev/null 2>&1",
                Self::shell_single_quote(&shell),
                Self::shell_single_quote(&session_name)
            );
            let right_cmd = format!(
                "less -R -- {}",
                Self::shell_single_quote(&selected_file)
            );
            let target_window = format!("{}:0", session_name);
            let target_left = format!("{}:0.0", session_name);

            let create_status = Command::new("tmux")
                .args(["new-session", "-d", "-s", &session_name, "-c", &current_dir, &left_cmd])
                .status()?;
            if !create_status.success() {
                return Err(io::Error::other("tmux new-session failed"));
            }

            let split_status = Command::new("tmux")
                .args(["split-window", "-h", "-p", "30", "-t", &target_window, "-c", &current_dir, &right_cmd])
                .status()?;
            if !split_status.success() {
                let _ = Command::new("tmux").args(["kill-session", "-t", &session_name]).status();
                return Err(io::Error::other("tmux split-window failed"));
            }

            let _ = Command::new("tmux")
                .args(["select-pane", "-t", &target_left])
                .status();

            let _ = Command::new("tmux")
                .args(["attach-session", "-t", &session_name])
                .status();

            let _ = Command::new("tmux")
                .args(["kill-session", "-t", &session_name])
                .status();

            Ok(())
        })();

        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        enable_raw_mode()?;
        execute!(io::stdout(), Hide)?;

        match tmux_result {
            Ok(()) => self.set_status("returned from split shell"),
            Err(e) => self.set_status(format!("split shell failed: {}", e)),
        }
        self.refresh_entries_or_status();
        Ok(())
    }

    fn open_split_shell_with_editor(&mut self) -> io::Result<()> {
        if !self.integration_active("tmux") {
            self.set_status("tmux not found in PATH");
            return Ok(());
        }

        let Some(entry) = self.entries.get(self.selected_index) else {
            self.set_status("no selected item");
            return Ok(());
        };

        let selected_path = entry.path();
        if selected_path.is_dir() {
            self.set_status("split shell edit works on files only");
            return Ok(());
        }

        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let editor = env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
        let current_dir = self.current_dir.to_string_lossy().into_owned();
        let selected_file = selected_path.to_string_lossy().into_owned();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let session_name = format!("sbrs_E_{}_{}", std::process::id(), stamp % 1_000_000_000);

        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        execute!(io::stdout(), Show)?;

        let tmux_result = (|| -> io::Result<()> {
            let left_cmd = format!(
                "{} -i; tmux kill-session -t {} >/dev/null 2>&1",
                Self::shell_single_quote(&shell),
                Self::shell_single_quote(&session_name)
            );
            let right_cmd = format!(
                "{} -- {}",
                editor,
                Self::shell_single_quote(&selected_file)
            );
            let target_window = format!("{}:0", session_name);
            let target_left = format!("{}:0.0", session_name);

            let create_status = Command::new("tmux")
                .args(["new-session", "-d", "-s", &session_name, "-c", &current_dir, &left_cmd])
                .status()?;
            if !create_status.success() {
                return Err(io::Error::other("tmux new-session failed"));
            }

            let split_status = Command::new("tmux")
                .args(["split-window", "-h", "-p", "30", "-t", &target_window, "-c", &current_dir, &right_cmd])
                .status()?;
            if !split_status.success() {
                let _ = Command::new("tmux").args(["kill-session", "-t", &session_name]).status();
                return Err(io::Error::other("tmux split-window failed"));
            }

            let _ = Command::new("tmux")
                .args(["select-pane", "-t", &target_left])
                .status();

            let _ = Command::new("tmux")
                .args(["attach-session", "-t", &session_name])
                .status();

            let _ = Command::new("tmux")
                .args(["kill-session", "-t", &session_name])
                .status();

            Ok(())
        })();

        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        enable_raw_mode()?;
        execute!(io::stdout(), Hide)?;

        match tmux_result {
            Ok(()) => self.set_status("returned from split shell"),
            Err(e) => self.set_status(format!("split shell failed: {}", e)),
        }
        self.refresh_entries_or_status();
        Ok(())
    }

    fn run_shell_command_and_wait_key(&mut self, command: &str) -> io::Result<()> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            self.set_status("command cancelled");
            return Ok(());
        }

        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;

        println!("$ {}", trimmed);
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = Command::new(&shell);
        // Non-interactive mode avoids shell job-control side effects that can
        // suspend sbrs when returning from the command runner.
        cmd.args(["-c", trimmed]);

        let status = cmd.current_dir(&self.current_dir).status();

        match status {
            Ok(s) => {
                if let Some(code) = s.code() {
                    println!("\n[exit code: {}]", code);
                } else {
                    println!("\n[process terminated by signal]");
                }
            }
            Err(e) => {
                println!("\n[failed to execute command: {}]", e);
            }
        }

        println!("\nPress Enter to return to sbrs...");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);

        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        enable_raw_mode()?;

        self.set_status(format!("ran command: {}", trimmed));
        self.refresh_entries_or_status();
        Ok(())
    }


    fn run_delta_compare(&mut self) -> io::Result<()> {
        if !self.integration_active("delta") {
            self.set_status("delta not found in PATH");
            return Ok(());
        }

        if self.marked_indices.len() != 1 {
            self.set_status("mark exactly one file, then move cursor to another file and press C");
            return Ok(());
        }

        let marked_idx = *self.marked_indices.iter().next().unwrap_or(&self.selected_index);
        let Some(marked_entry) = self.entries.get(marked_idx) else {
            self.set_status("marked file not found");
            return Ok(());
        };
        let Some(cursor_entry) = self.entries.get(self.selected_index) else {
            self.set_status("cursor file not found");
            return Ok(());
        };

        let marked_path = marked_entry.path();
        let cursor_path = cursor_entry.path();

        if marked_path == cursor_path {
            self.set_status("choose a different cursor file to compare");
            return Ok(());
        }
        if marked_path.is_dir() || cursor_path.is_dir() {
            self.set_status("delta compare works on files only");
            return Ok(());
        }

        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        let _ = Command::new("delta")
            .arg("--side-by-side")
            .arg("--paging=always")
            .arg(&marked_path)
            .arg(&cursor_path)
            .status();
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        enable_raw_mode()?;

        let left = marked_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| marked_path.to_string_lossy().into_owned());
        let right = cursor_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| cursor_path.to_string_lossy().into_owned());
        self.set_status(format!("delta compared: {} vs {}", left, right));
        Ok(())
    }

    fn open_selected_with_default_app(&mut self) -> io::Result<()> {
        let entry = if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.entries.get(self.right.selected_index)
        } else {
            self.entries.get(self.selected_index)
        };
        let Some(entry) = entry else {
            self.set_status("no selected item");
            return Ok(());
        };

        let path = entry.path();
        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        #[cfg(target_os = "macos")]
        let opened = if Self::integration_probe("open").0 {
            Command::new("open")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        } else {
            false
        };

        #[cfg(not(target_os = "macos"))]
        let opened = if Self::integration_probe("xdg-open").0 {
            Command::new("xdg-open")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        } else if Self::integration_probe("gio").0 {
            Command::new("gio")
                .arg("open")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        } else {
            false
        };

        if opened {
            self.set_status(format!("opened with default app: {}", display_name));
        } else {
            #[cfg(target_os = "macos")]
            self.set_status("no default opener found (tried open)");

            #[cfg(not(target_os = "macos"))]
            self.set_status("no default opener found (tried xdg-open, gio open)");
        }

        Ok(())
    }

    fn open_todo_file_in_editor(&mut self) -> io::Result<()> {
        let home = match env::var("HOME") {
            Ok(v) => v,
            Err(_) => {
                self.set_status("HOME is not set");
                return Ok(());
            }
        };

        let todo_path = PathBuf::from(home).join(".todo");
        if !todo_path.exists() {
            if let Err(e) = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&todo_path)
            {
                self.set_status(format!("failed to create ~/.todo: {}", e));
                return Ok(());
            }
        }

        let editor = env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        execute!(io::stdout(), Show)?;
        let _ = Command::new(editor).arg(&todo_path).status();
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        execute!(io::stdout(), Hide)?;
        enable_raw_mode()?;
        self.refresh_entries_or_status();
        self.set_status("opened ~/.todo");
        Ok(())
    }






    fn is_path_inside_remote_mount(&self, path: &PathBuf) -> bool {
        self.ssh_mounts
            .iter()
            .any(|m| path == &m.mount_path || path.starts_with(&m.mount_path))
    }

    fn begin_transfer_from_sources(
        &mut self,
        sources: Vec<PathBuf>,
        target_dir: PathBuf,
        move_mode: bool,
    ) {
        if sources.is_empty() {
            self.set_status("no selected item");
            return;
        }
        if self.archive_rx.is_some() {
            self.set_status("archive creation in progress");
            return;
        }
        if self.copy_rx.is_some() {
            self.set_status("copy already in progress");
            return;
        }
        self.paste_queue = sources.iter().cloned().collect();
        self.paste_current_src = None;
        self.paste_move_mode = move_mode;
        self.paste_target_dir = Some(target_dir);
        self.paste_total_items = sources.len();
        self.paste_ok_items = 0;
        self.paste_failed_items = 0;
        let (tx_total, rx_total) = mpsc::channel();
        self.copy_total_rx = Some(rx_total);
        thread::spawn(move || {
            let total = sources
                .iter()
                .filter_map(|src| App::compute_total_bytes(src).ok())
                .fold(0u64, |acc, v| acc.saturating_add(v));
            let _ = tx_total.send(total);
        });
        self.copy_total_bytes = 0;
        self.copy_done_bytes = 0;
        self.copy_done_before_job = 0;
        self.copy_job_total_bytes = 0;
        self.copy_started_at = Some(Instant::now());
        self.copy_current_src = None;
        self.advance_paste_queue();
    }

    fn begin_transfer(&mut self, move_mode: bool) {
        if self.clipboard.is_empty() {
            self.set_status("clipboard is empty");
            return;
        }
        self.begin_transfer_from_sources(self.clipboard.clone(), self.current_dir.clone(), move_mode);
    }

    fn begin_dual_panel_transfer(&mut self, move_mode: bool) {
        if !self.is_dual_panel_mode() {
            self.set_status("dual panel mode is not active");
            return;
        }

        let (sources, target_dir) = match self.active_panel {
            DualPanelSide::Left => {
                let sources = if !self.marked_indices.is_empty() {
                    self.entries
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| self.marked_indices.contains(i))
                        .map(|(_, e)| e.path())
                        .collect()
                } else {
                    self.entries
                        .get(self.selected_index)
                        .map(|e| vec![e.path()])
                        .unwrap_or_default()
                };
                (sources, self.right.dir.clone())
            }
            DualPanelSide::Right => {
                let sources = if !self.right.marked_indices.is_empty() {
                    self.right.entries
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| self.right.marked_indices.contains(i))
                        .map(|(_, e)| e.path())
                        .collect()
                } else {
                    self.right.entries
                        .get(self.right.selected_index)
                        .map(|e| vec![e.path()])
                        .unwrap_or_default()
                };
                (sources, self.current_dir.clone())
            }
        };

        self.begin_transfer_from_sources(sources, target_dir, move_mode);
    }

    fn pump_copy_total_prescan(&mut self) {
        let Some(rx) = self.copy_total_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(total) => {
                self.copy_total_bytes = total;
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.copy_total_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.copy_total_rx = None;
            }
        }
    }

    fn begin_paste(&mut self) {
        self.begin_transfer(false);
    }

    fn begin_move(&mut self) {
        self.begin_transfer(true);
    }

    fn copy_full_paths_to_system_clipboard(&mut self) {
        let targets = self.delete_targets();
        if targets.is_empty() {
            self.set_status("no selected item");
            return;
        }

        let payload = targets
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");

        for backend in ["wl-copy", "xclip", "xsel", "pbcopy"] {
            if !self.integration_active(backend) {
                continue;
            }

            let mut cmd = match backend {
                "wl-copy" => Command::new("wl-copy"),
                "xclip" => {
                    let mut cmd = Command::new("xclip");
                    cmd.args(["-selection", "clipboard"]);
                    cmd
                }
                "xsel" => {
                    let mut cmd = Command::new("xsel");
                    cmd.args(["--clipboard", "--input"]);
                    cmd
                }
                "pbcopy" => Command::new("pbcopy"),
                _ => continue,
            };

            let mut child = match cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => continue,
            };

            let write_ok = child
                .stdin
                .take()
                .map(|mut stdin| stdin.write_all(payload.as_bytes()).is_ok())
                .unwrap_or(false);
            if !write_ok {
                let _ = child.kill();
                let _ = child.wait();
                continue;
            }

            if child.wait().map(|s| s.success()).unwrap_or(false) {
                self.set_status(format!(
                    "copied {} full path(s) to system clipboard via {}",
                    targets.len(),
                    backend
                ));
                return;
            }
        }

        self.set_status("no clipboard backend available (wl-copy/xclip/xsel/pbcopy)");
    }

    fn read_system_clipboard_text(&self) -> Option<(String, &'static str)> {
        for backend in ["wl-copy", "xclip", "xsel", "pbcopy"] {
            if !self.integration_active(backend) {
                continue;
            }

            let output = match backend {
                "wl-copy" => {
                    if !Self::integration_probe("wl-paste").0 {
                        continue;
                    }
                    Command::new("wl-paste")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output()
                }
                "xclip" => Command::new("xclip")
                    .args(["-selection", "clipboard", "-out"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output(),
                "xsel" => Command::new("xsel")
                    .args(["--clipboard", "--output"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output(),
                "pbcopy" => {
                    if !Self::integration_probe("pbpaste").0 {
                        continue;
                    }
                    Command::new("pbpaste")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output()
                }
                _ => continue,
            };

            if let Ok(out) = output {
                if out.status.success() {
                    return Some((String::from_utf8_lossy(&out.stdout).into_owned(), backend));
                }
            }
        }

        None
    }

    fn write_system_clipboard_text(&self, payload: &str) -> Option<&'static str> {
        for backend in ["wl-copy", "xclip", "xsel", "pbcopy"] {
            if !self.integration_active(backend) {
                continue;
            }

            let mut cmd = match backend {
                "wl-copy" => Command::new("wl-copy"),
                "xclip" => {
                    let mut cmd = Command::new("xclip");
                    cmd.args(["-selection", "clipboard"]);
                    cmd
                }
                "xsel" => {
                    let mut cmd = Command::new("xsel");
                    cmd.args(["--clipboard", "--input"]);
                    cmd
                }
                "pbcopy" => Command::new("pbcopy"),
                _ => continue,
            };

            let mut child = match cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => continue,
            };

            let write_ok = child
                .stdin
                .take()
                .map(|mut stdin| stdin.write_all(payload.as_bytes()).is_ok())
                .unwrap_or(false);
            if !write_ok {
                let _ = child.kill();
                let _ = child.wait();
                continue;
            }

            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Some(backend);
            }
        }

        None
    }

    fn edit_system_clipboard_via_temp_file(&mut self) -> io::Result<()> {
        let Some((clipboard_text, read_backend)) = self.read_system_clipboard_text() else {
            self.set_status("no clipboard backend available (wl-copy/xclip/xsel/pbcopy)");
            return Ok(());
        };

        let tmp = Self::create_temp_selection_path("sbrs_clipboard_edit");
        if fs::write(&tmp, clipboard_text.as_bytes()).is_err() {
            self.set_status("failed to create temporary clipboard file");
            return Ok(());
        }

        disable_raw_mode()?;
        execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        execute!(io::stdout(), Show)?;

        let edit_result = (|| -> io::Result<String> {
            let _ = Command::new(env::var("EDITOR").unwrap_or_else(|_| "nano".to_string()))
                .arg(&tmp)
                .status();
            fs::read_to_string(&tmp)
        })();

        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        enable_raw_mode()?;
        execute!(io::stdout(), Hide)?;

        let _ = fs::remove_file(&tmp);

        match edit_result {
            Ok(updated_text) => {
                if let Some(write_backend) = self.write_system_clipboard_text(&updated_text) {
                    self.set_status(format!(
                        "clipboard updated via {} (read via {})",
                        write_backend, read_backend
                    ));
                } else {
                    self.set_status("failed to write updated clipboard content");
                }
            }
            Err(e) => {
                self.set_status(format!("clipboard edit failed: {}", e));
            }
        }

        Ok(())
    }

    fn copy_path_with_progress(
        src: &PathBuf,
        dest: &PathBuf,
        tx: &Sender<CopyProgressMsg>,
        copied_bytes: &mut u64,
    ) -> io::Result<()> {
        if src.is_dir() {
            fs::create_dir_all(dest)?;
            for child in fs::read_dir(src)? {
                let child = child?;
                let child_src = child.path();
                let child_dest = dest.join(child.file_name());
                Self::copy_path_with_progress(&child_src, &child_dest, tx, copied_bytes)?;
            }
            Ok(())
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut in_file = fs::File::open(src)?;
            let mut out_file = fs::File::create(dest)?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = in_file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                out_file.write_all(&buffer[..read])?;
                *copied_bytes = copied_bytes.saturating_add(read as u64);
                let _ = tx.send(CopyProgressMsg::CopiedBytes(*copied_bytes));
            }
            Ok(())
        }
    }

    fn update_copy_status(&mut self) {
        if self.copy_item_name.is_empty() {
            return;
        }
        let total = self.copy_total_bytes;
        let scanning = total == 0 && self.copy_total_rx.is_some();
        let done = if total == 0 {
            self.copy_done_bytes
        } else {
            self.copy_done_bytes.min(total)
        };
        let effective_total = if total == 0 {
            done
                .saturating_add(self.copy_job_total_bytes)
                .max(1)
        } else {
            total.max(1)
        };
        let percent = if total == 0 {
            if self.copy_total_rx.is_some() { 0.0 } else { 100.0 }
        } else {
            (done as f64 * 100.0) / effective_total as f64
        };
        let elapsed_secs = self
            .copy_started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0)
            .max(0.001);
        let bytes_per_sec = done as f64 / elapsed_secs;
        let remaining = if total == 0 { 0 } else { total.saturating_sub(done) };
        let eta_secs = if bytes_per_sec > 0.0 {
            (remaining as f64 / bytes_per_sec) as u64
        } else {
            0
        };
        let bar_width = 14usize;
        let filled = ((percent / 100.0) * bar_width as f64).round() as usize;
        let bar = format!(
            "{}{}",
            "#".repeat(filled.min(bar_width)),
            "-".repeat(bar_width.saturating_sub(filled.min(bar_width)))
        );
        let total_label = if total == 0 && self.copy_total_rx.is_some() {
            "?".to_string()
        } else {
            Self::format_size(effective_total)
        };
        let eta_label = if total == 0 { "-".to_string() } else { Self::format_eta(eta_secs) };
        let scan_suffix = if scanning { " scanning size..." } else { "" };
        let current_idx = (self.paste_ok_items + self.paste_failed_items + 1).min(self.paste_total_items.max(1));
        let scope = if self.copy_from_remote { "remote " } else { "" };
        self.set_status(format!(
            "{}copy [{}] {:>3.0}% {}/{} {}/s eta {} ({}/{}) {}{}",
            scope,
            bar,
            percent,
            Self::format_size(done),
            total_label,
            Self::format_size(bytes_per_sec as u64),
            eta_label,
            current_idx,
            self.paste_total_items,
            self.copy_item_name,
            scan_suffix
        ));
    }

    fn start_copy_job(&mut self, src: PathBuf, dest: PathBuf, display_name: String) {
        let (tx, rx) = mpsc::channel();
        self.copy_rx = Some(rx);
        self.copy_done_before_job = self.copy_done_bytes;
        self.copy_job_total_bytes = 0;
        self.copy_item_name = display_name;
        self.copy_current_src = Some(src.clone());
        self.copy_from_remote = self.is_path_inside_remote_mount(&src);
        self.update_copy_status();

        thread::spawn(move || {
            let total = Self::compute_total_bytes(&src).unwrap_or(0);
            let _ = tx.send(CopyProgressMsg::TotalBytes(total));
            let mut copied = 0u64;
            let result = Self::copy_path_with_progress(&src, &dest, &tx, &mut copied)
                .map_err(|e| e.to_string());
            let _ = tx.send(CopyProgressMsg::Finished(result));
        });
    }

    fn pump_copy_progress(&mut self) {
        let Some(rx) = self.copy_rx.take() else {
            return;
        };

        let mut done_result: Option<Result<(), String>> = None;
        loop {
            match rx.try_recv() {
                Ok(CopyProgressMsg::TotalBytes(total)) => {
                    self.copy_job_total_bytes = total;
                }
                Ok(CopyProgressMsg::CopiedBytes(done)) => {
                    self.copy_done_bytes = self.copy_done_before_job.saturating_add(done);
                }
                Ok(CopyProgressMsg::Finished(result)) => {
                    done_result = Some(result);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    done_result = Some(Err("copy worker disconnected".to_string()));
                    break;
                }
            }
        }

        if let Some(result) = done_result {
            match result {
                Ok(()) => {
                    if self.paste_move_mode {
                        if let Some(src) = self.copy_current_src.take() {
                            let delete_res = if src.is_dir() {
                                fs::remove_dir_all(&src)
                            } else {
                                fs::remove_file(&src)
                            };
                            if let Err(e) = delete_res {
                                self.paste_failed_items += 1;
                                self.set_status(format!("move cleanup failed for {}: {}", self.copy_item_name, e));
                                self.copy_job_total_bytes = 0;
                                self.copy_done_before_job = self.copy_done_bytes;
                                self.copy_item_name.clear();
                                self.copy_from_remote = false;
                                let _ = self.refresh_entries();
                                if self.is_dual_panel_mode() {
                                    let _ = self.refresh_right_panel_entries();
                                }
                                self.advance_paste_queue();
                                return;
                            }
                        }
                    }
                    self.paste_ok_items += 1;
                    self.copy_done_bytes = self
                        .copy_done_before_job
                        .saturating_add(self.copy_job_total_bytes);
                }
                Err(e) => {
                    self.paste_failed_items += 1;
                    self.set_status(format!("paste failed for {}: {}", self.copy_item_name, e));
                }
            }
            self.copy_job_total_bytes = 0;
            self.copy_done_before_job = self.copy_done_bytes;
            self.copy_item_name.clear();
            self.copy_current_src = None;
            self.copy_from_remote = false;
            let _ = self.refresh_entries();
            if self.is_dual_panel_mode() {
                let _ = self.refresh_right_panel_entries();
            }
            self.advance_paste_queue();
        } else {
            self.copy_rx = Some(rx);
            self.update_copy_status();
        }
    }

    fn format_eta(total_seconds: u64) -> String {
        util::format::format_eta(total_seconds)
    }

    fn advance_paste_queue(&mut self) {
        if self.copy_rx.is_some() {
            return;
        }
        while let Some(src) = self.paste_queue.pop_front() {
            let name = src
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "pasted_item".to_string());
            let target_dir = self
                .paste_target_dir
                .as_ref()
                .cloned()
                .unwrap_or_else(|| self.current_dir.clone());
            let dest = target_dir.join(&name);
            if dest.exists() {
                self.paste_current_src = Some(src);
                self.begin_input_edit(AppMode::PasteRenaming, name);
                self.set_status("target exists: edit name and press Enter");
                return;
            }

            if self.paste_move_mode {
                if fs::rename(&src, &dest).is_ok() {
                    self.paste_ok_items += 1;
                    let _ = self.refresh_entries();
                    if self.is_dual_panel_mode() {
                        let _ = self.refresh_right_panel_entries();
                    }
                    continue;
                }
            }

            self.start_copy_job(src, dest, name);
            return;
        }

        self.paste_current_src = None;
        self.paste_move_mode = false;
        self.paste_target_dir = None;
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
        self.copy_started_at = None;
        self.copy_total_rx = None;
        self.copy_current_src = None;
        self.refresh_entries_or_status();
        if self.is_dual_panel_mode() {
            let _ = self.refresh_right_panel_entries();
        }
        if self.paste_failed_items == 0 && self.paste_ok_items > 0 {
            self.set_status(format!("transfer complete: {} item", self.paste_ok_items));
        } else if self.paste_failed_items == 0 {
            self.set_status("nothing to transfer");
        } else {
            self.set_status(format!(
                "transfer finished: {} ok, {} failed ({} total)",
                self.paste_ok_items, self.paste_failed_items, self.paste_total_items
            ));
        }
    }

    fn panel_tab_bar_line(active: u8, theme_id: ui::theme::ThemeId) -> Line<'static> {
        ui::panels::panel_tab_bar_line(active, theme_id)
    }

    fn panel_tab_hit_test(relative_x: u16) -> Option<u8> {
        ui::panels::panel_tab_hit_test(relative_x)
    }

    fn tabbed_overlay_close_area(popup_area: Rect) -> Rect {
        Rect::new(
            popup_area.x + popup_area.width.saturating_sub(2),
            popup_area.y,
            1,
            1,
        )
    }

    fn primary_content_area(area: Rect) -> Rect {
        Layout::default()
            .constraints([Constraint::Min(3), Constraint::Length(2)])
            .split(area)[0]
    }

    fn tab_overlay_anchor(area: Rect) -> Rect {
        let area = Self::primary_content_area(area);
        let anchor_w = (area.width * 5 / 6).max(50).min(area.width);
        let anchor_h = (area.height * 5 / 6).max(12).min(area.height);
        Rect::new(
            area.x + (area.width.saturating_sub(anchor_w)) / 2,
            area.y + (area.height.saturating_sub(anchor_h)) / 2,
            anchor_w,
            anchor_h,
        )
    }

    fn open_panel_tab(&mut self, tab: u8) {
        if tab == self.panel_tab
            && matches!(
                (tab, self.mode),
                (0, AppMode::Help)
                    | (1, AppMode::InternalSearch)
                    | (2, AppMode::Bookmarks)
                    | (3, AppMode::SshPicker)
                    | (4, AppMode::SortMenu)
                    | (5, AppMode::Integrations)
                    | (6, AppMode::Themes)
            )
        {
            return;
        }

        match tab {
            0 => {
                self.panel_tab = 0;
                self.help_scroll_offset = 0;
                self.mode = AppMode::Help;
            }
            1 => {
                self.panel_tab = 1;
                self.start_internal_search();
            }
            2 => {
                self.panel_tab = 2;
                self.mode = AppMode::Bookmarks;
            }
            3 => {
                self.panel_tab = 3;
                self.refresh_remote_entries();
                self.mode = AppMode::SshPicker;
            }
            4 => {
                self.begin_sort_menu();
            }
            5 => {
                self.integration_selected = 0;
                self.refresh_integration_rows_cache();
                self.panel_tab = 5;
                self.mode = AppMode::Integrations;
            }
            6 => {
                self.theme_selected = ui::theme::THEMES
                    .iter()
                    .position(|theme| theme.id == self.active_theme)
                    .unwrap_or(0);
                self.panel_tab = 6;
                self.mode = AppMode::Themes;
            }
            _ => {}
        }
    }

    fn close_tabbed_overlay(&mut self) {
        match self.mode {
            AppMode::InternalSearch => {
                self.cancel_internal_search_candidate_scan();
                self.cancel_internal_search_content_request();
                self.clear_input_edit();
                self.mode = AppMode::Browsing;
            }
            AppMode::Help
            | AppMode::Bookmarks
            | AppMode::Integrations
            | AppMode::Themes
            | AppMode::SortMenu
            | AppMode::SshPicker => {
                self.mode = AppMode::Browsing;
            }
            _ => {}
        }
    }

    fn handle_tab_close_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if !matches!(
            self.mode,
            AppMode::InternalSearch
                | AppMode::Help
                | AppMode::Bookmarks
                | AppMode::Integrations
                | AppMode::Themes
                | AppMode::SortMenu
                | AppMode::SshPicker
        ) {
            return false;
        }

        let popup_area = Self::tab_overlay_anchor(area);
        let close_area = Self::tabbed_overlay_close_area(popup_area);
        if row == close_area.y && column >= close_area.x && column < close_area.x + close_area.width {
            self.close_tabbed_overlay();
            return true;
        }

        false
    }

    fn handle_tab_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if !matches!(
            self.mode,
            AppMode::InternalSearch
                | AppMode::Help
                | AppMode::Bookmarks
                | AppMode::Integrations
                | AppMode::Themes
                | AppMode::SortMenu
                | AppMode::SshPicker
        ) {
            return false;
        }

        let popup_area = Self::tab_overlay_anchor(area);
        if row != popup_area.y || column <= popup_area.x || column >= popup_area.x + popup_area.width.saturating_sub(1) {
            return false;
        }

        let relative_x = column.saturating_sub(popup_area.x + 1);
        if let Some(tab) = Self::panel_tab_hit_test(relative_x) {
            self.open_panel_tab(tab);
            return true;
        }

        false
    }

    fn handle_confirm_delete_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if self.mode != AppMode::ConfirmDelete {
            return false;
        }

        let to_delete = self.delete_targets();
        let (mut file_count, mut folder_count) = (0usize, 0usize);
        for path in &to_delete {
            if path.is_dir() {
                folder_count += 1;
            } else {
                file_count += 1;
            }
        }
        let title = ui::dialogs::confirm_delete_title(file_count, folder_count);
        let confirm_area = ui::dialogs::confirm_delete_dialog_area(area, &title);
        let Some((button_area, confirm_start, confirm_w, cancel_start, cancel_w)) =
            ui::dialogs::confirm_delete_button_layout(confirm_area)
        else {
            return false;
        };

        if row != button_area.y {
            return false;
        }

        if column >= confirm_start && column < confirm_start + confirm_w {
            self.confirm_delete_button_focus = 0;
            self.confirm_delete_selected_targets();
            return true;
        }
        if column >= cancel_start && column < cancel_start + cancel_w {
            self.confirm_delete_button_focus = 1;
            self.mode = AppMode::Browsing;
            return true;
        }

        false
    }

    fn handle_confirm_integration_install_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        if self.mode != AppMode::ConfirmIntegrationInstall {
            return false;
        }

        let Some((button_area, ok_start, ok_w, cancel_start, cancel_w)) =
            self.confirm_integration_install_button_layout(area)
        else {
            return false;
        };

        if row != button_area.y {
            return false;
        }

        if column >= ok_start && column < ok_start + ok_w {
            self.confirm_integration_install_button_focus = 0;
            return self.confirm_integration_install().is_ok();
        }
        if column >= cancel_start && column < cancel_start + cancel_w {
            self.confirm_integration_install_button_focus = 1;
            self.mode = AppMode::Integrations;
            self.clear_integration_install_prompt();
            self.set_status("integration install cancelled");
            return true;
        }

        false
    }

    fn confirm_integration_install_msg_lines(&self) -> Vec<String> {
        let key = self
            .integration_install_key
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string());
        let package = self
            .integration_install_package
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string());
        let brew_display = self
            .integration_install_brew_path
            .clone()
            .unwrap_or_else(|| "brew (not found)".to_string());

        ui::dialogs::confirm_integration_install_msg_lines(
            &key,
            &package,
            &brew_display,
            self.integration_install_brew_path.is_none(),
        )
    }

    fn confirm_integration_install_dialog_area(&self, area: Rect) -> Rect {
        let msg_lines = self.confirm_integration_install_msg_lines();
        ui::dialogs::confirm_integration_install_dialog_area(area, &msg_lines)
    }

    fn confirm_integration_install_button_layout(
        &self,
        area: Rect,
    ) -> Option<(Rect, u16, u16, u16, u16)> {
        let confirm_area = self.confirm_integration_install_dialog_area(area);
        ui::dialogs::confirm_ok_cancel_button_layout(confirm_area)
    }

    fn inner_with_borders(area: Rect) -> Rect {
        Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        )
    }

    fn internal_search_header_rows(&self) -> usize {
        let mut rows = 0usize;
        if self.internal_search_candidates_pending || self.internal_search_candidates_truncated {
            rows += 1;
        }

        if self.internal_search_scope == InternalSearchScope::Content {
            rows += 1; // limits summary
            if self.internal_search_limits_menu_open {
                rows += 4; // 3 editable rows + helper line
            } else {
                rows += 1; // open editor hint
            }
            if self.internal_search_content_pending {
                rows += 1;
            }
            if self.internal_search_content_limit_note.is_some() {
                rows += 1;
            }
        }

        rows
    }

    fn clickable_key_from_tabbed_row(
        &mut self,
        column: u16,
        row: u16,
        area: Rect,
    ) -> Option<KeyEvent> {
        match self.mode {
            AppMode::InternalSearch => {
                if self.internal_search_results.is_empty() {
                    return None;
                }

                let popup_area = Self::tab_overlay_anchor(area);
                let popup_inner = Self::inner_with_borders(popup_area);
                let search_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(1),
                        Constraint::Length(2),
                    ])
                    .split(popup_inner);
                let body_area = search_layout[1];

                if row < body_area.y || row >= body_area.y + body_area.height {
                    return None;
                }
                if column < body_area.x || column >= body_area.x + body_area.width {
                    return None;
                }

                let header_rows = self.internal_search_header_rows();
                let regex_rows = usize::from(self.internal_search_regex_error.is_some());
                let visible_rows = body_area.height as usize;
                let max_rows = visible_rows.saturating_sub(header_rows).max(1);
                let offset = if self.internal_search_selected >= max_rows {
                    self.internal_search_selected + 1 - max_rows
                } else {
                    0
                };

                let result_start_y = body_area
                    .y
                    .saturating_add((header_rows + regex_rows) as u16);
                if row < result_start_y {
                    return None;
                }

                let clicked_result_row = row.saturating_sub(result_start_y) as usize;
                let rendered_results = self
                    .internal_search_results
                    .len()
                    .saturating_sub(offset)
                    .min(max_rows);
                if clicked_result_row >= rendered_results {
                    return None;
                }

                self.internal_search_selected = offset + clicked_result_row;
                Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            }
            AppMode::Bookmarks => {
                let overlay = Self::tab_overlay_anchor(area);
                let bookmarks = Self::load_bookmarks();
                if bookmarks.is_empty() {
                    return None;
                }

                let bm_w = (area.width * 2 / 3).max(50).min(overlay.width);
                let mut line_count = 1usize + bookmarks.len();
                line_count += 4; // trailing helper lines
                let bm_h = (line_count as u16 + 4).max(17).min(overlay.height);
                let bm_area = Rect::new(overlay.x, overlay.y, bm_w, bm_h);
                let bm_inner = Self::inner_with_borders(bm_area);
                let bm_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(bm_inner);
                let content = bm_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let line_idx = row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= bookmarks.len() {
                    self.bookmark_selected = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }

                None
            }
            AppMode::Integrations => {
                let overlay = Self::tab_overlay_anchor(area);
                let integrations_len = self.integration_rows_cache.len();
                if integrations_len == 0 {
                    return None;
                }

                let int_w = (area.width * 5 / 6).max(70).min(overlay.width);
                let int_h = (integrations_len as u16 + 1 + 4).min(overlay.height);
                let int_area = Rect::new(overlay.x, overlay.y, int_w, int_h);
                let int_inner = Self::inner_with_borders(int_area);
                let int_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(int_inner);
                let content = int_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let visible_rows = content.height as usize;
                let selected_line = self.integration_selected + 1;
                let int_scroll = if selected_line + 1 <= visible_rows {
                    0usize
                } else {
                    selected_line + 1 - visible_rows
                };
                let line_idx = int_scroll + row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= integrations_len {
                    self.integration_selected = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
                }

                None
            }
            AppMode::SshPicker => {
                if self.remote_entries.is_empty() {
                    return None;
                }

                let overlay = Self::tab_overlay_anchor(area);
                let ssh_w = (area.width * 2 / 3).max(60).min(area.width);
                let ssh_popup_w = ssh_w.min(overlay.width);
                let lines_len = 1usize + self.remote_entries.len();
                let ssh_h = (lines_len as u16 + 4).max(8).min(overlay.height);
                let ssh_area = Rect::new(overlay.x, overlay.y, ssh_popup_w, ssh_h);
                let ssh_inner = Self::inner_with_borders(ssh_area);
                let ssh_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(ssh_inner);
                let content = ssh_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let line_idx = row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= self.remote_entries.len() {
                    self.ssh_picker_selection = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }

                None
            }
            AppMode::SortMenu => {
                let overlay = Self::tab_overlay_anchor(area);
                let options = Self::sort_mode_options();
                if options.is_empty() {
                    return None;
                }

                let sort_w = overlay.width;
                let line_count = 1usize + options.len();
                let sort_h = (line_count as u16 + 4).max(10).min(overlay.height);
                let sort_area = Rect::new(overlay.x, overlay.y, sort_w, sort_h);
                let sort_inner = Self::inner_with_borders(sort_area);
                let sort_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(sort_inner);
                let content = sort_chunks[0];

                if row < content.y || row >= content.y + content.height {
                    return None;
                }
                if column < content.x || column >= content.x + content.width {
                    return None;
                }

                let line_idx = row.saturating_sub(content.y) as usize;
                if line_idx >= 1 && line_idx <= options.len() {
                    self.sort_menu_selected = line_idx - 1;
                    return Some(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }

                None
            }
            _ => None,
        }
    }

    fn handle_mouse_scroll(&mut self, scroll_up: bool) {
        match self.mode {
            AppMode::Browsing => {
                if self.preview_focus_is_preview() {
                    if scroll_up {
                        self.preview_scroll_up(3);
                    } else {
                        self.preview_scroll_down(3);
                    }
                } else {
                    let delta = if scroll_up { -3 } else { 3 };
                    self.move_selection_delta(delta);
                }
            }
            AppMode::Help => {
                if scroll_up {
                    self.help_scroll_offset = self.help_scroll_offset.saturating_sub(3);
                } else {
                    self.help_scroll_offset = (self.help_scroll_offset + 3).min(self.help_max_offset);
                }
            }
            AppMode::InternalSearch => {
                if self.internal_search_limits_menu_open {
                    if scroll_up {
                        self.internal_search_limits_selected = self.internal_search_limits_selected.saturating_sub(1);
                    } else {
                        self.internal_search_limits_selected = (self.internal_search_limits_selected + 1).min(2);
                    }
                } else if !self.internal_search_results.is_empty() {
                    if scroll_up {
                        self.internal_search_selected = self.internal_search_selected.saturating_sub(1);
                    } else {
                        self.internal_search_selected = (self.internal_search_selected + 1)
                            .min(self.internal_search_results.len().saturating_sub(1));
                    }
                }
            }
            AppMode::Bookmarks => {
                let max_idx = Self::load_bookmarks().len().saturating_sub(1);
                if scroll_up {
                    self.bookmark_selected = self.bookmark_selected.saturating_sub(1);
                } else {
                    self.bookmark_selected = (self.bookmark_selected + 1).min(max_idx);
                }
            }
            AppMode::Integrations => {
                let max_idx = self.integration_count().saturating_sub(1);
                if scroll_up {
                    self.integration_selected = self.integration_selected.saturating_sub(1);
                } else {
                    self.integration_selected = (self.integration_selected + 1).min(max_idx);
                }
            }
            AppMode::SortMenu => {
                let max_idx = Self::sort_mode_options().len().saturating_sub(1);
                if scroll_up {
                    self.sort_menu_selected = self.sort_menu_selected.saturating_sub(1);
                } else {
                    self.sort_menu_selected = (self.sort_menu_selected + 1).min(max_idx);
                }
            }
            AppMode::SshPicker => {
                let max_idx = self.remote_entries.len().saturating_sub(1);
                if scroll_up {
                    self.ssh_picker_selection = self.ssh_picker_selection.saturating_sub(1);
                } else {
                    self.ssh_picker_selection = (self.ssh_picker_selection + 1).min(max_idx);
                }
            }
            AppMode::ConfirmDelete => {
                if scroll_up {
                    self.confirm_delete_scroll_offset = self.confirm_delete_scroll_offset.saturating_sub(3);
                } else {
                    self.confirm_delete_scroll_offset =
                        (self.confirm_delete_scroll_offset + 3).min(self.confirm_delete_max_offset);
                }
            }
            _ => {}
        }
    }

    fn main_table_and_list_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
        if self.mode != AppMode::Browsing {
            return None;
        }

        let footer_height = if self.is_preview_mode() || self.is_dual_panel_mode() { 1 } else { 2 };
        let header_reserved_rows = if self.is_preview_mode() || self.is_dual_panel_mode() { 1 } else { 2 };
        let chunks = Layout::default()
            .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
            .split(area);

        let content_area = Rect::new(
            chunks[0].x,
            chunks[0].y + header_reserved_rows,
            chunks[0].width,
            chunks[0].height.saturating_sub(header_reserved_rows),
        );

        let list_frame_area = if self.is_preview_mode() {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(33), Constraint::Percentage(67)])
                .split(content_area)[0]
        } else if self.is_dual_panel_mode() {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(content_area)[0]
        } else {
            content_area
        };

        let table_area = if self.is_preview_mode() || self.is_dual_panel_mode() {
            Rect::new(
                list_frame_area.x + 1,
                list_frame_area.y + 1,
                list_frame_area.width.saturating_sub(2),
                list_frame_area.height.saturating_sub(2),
            )
        } else {
            content_area
        };

        if table_area.height == 0 || table_area.width == 0 {
            return None;
        }

        let needs_scroll = self.entries.len() > table_area.height as usize;
        let can_draw_scrollbar = self.mode_shows_main_scrollbar() && table_area.width > 2 && needs_scroll;
        let list_area = if can_draw_scrollbar {
            Rect::new(
                table_area.x,
                table_area.y,
                table_area.width.saturating_sub(1),
                table_area.height,
            )
        } else {
            table_area
        };

        Some((table_area, list_area))
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

    fn right_table_and_list_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
        let (_, right_frame_area) = self.dual_panel_frame_areas(area)?;

        let table_area = Rect::new(
            right_frame_area.x + 1,
            right_frame_area.y + 1,
            right_frame_area.width.saturating_sub(2),
            right_frame_area.height.saturating_sub(2),
        );

        if table_area.height == 0 || table_area.width == 0 {
            return None;
        }

        let needs_scroll = self.right.entries.len() > table_area.height as usize;
        let can_draw_scrollbar = table_area.width > 2 && needs_scroll;
        let list_area = if can_draw_scrollbar {
            Rect::new(
                table_area.x,
                table_area.y,
                table_area.width.saturating_sub(1),
                table_area.height,
            )
        } else {
            table_area
        };

        Some((table_area, list_area))
    }

    fn preview_pane_frame_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
        if !self.is_preview_mode() || !matches!(self.mode, AppMode::Browsing | AppMode::PathEditing) {
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
            .constraints([Constraint::Percentage(33), Constraint::Percentage(67)])
            .split(content_area);
        Some((split[0], split[1]))
    }

    fn handle_preview_pane_tab_click(&mut self, column: u16, row: u16, area: Rect) -> bool {
        let Some((folder_area, preview_area)) = self.preview_pane_frame_areas(area) else {
            return false;
        };

        let in_folder = column >= folder_area.x
            && column < folder_area.x + folder_area.width
            && row >= folder_area.y
            && row < folder_area.y + folder_area.height;
        let in_preview = column >= preview_area.x
            && column < preview_area.x + preview_area.width
            && row >= preview_area.y
            && row < preview_area.y + preview_area.height;

        if in_folder {
            self.preview_pane_focus = PreviewPaneFocus::Folder;
            return false;
        }

        if in_preview {
            self.preview_pane_focus = PreviewPaneFocus::Preview;
            return true;
        }

        false
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

    fn main_table_scrollbar_area(&self, area: Rect) -> Option<Rect> {
        let (table_area, list_area) = self.main_table_and_list_areas(area)?;
        if list_area.width >= table_area.width || list_area.height == 0 {
            return None;
        }

        Some(Rect::new(
            list_area.x + list_area.width,
            list_area.y,
            1,
            list_area.height,
        ))
    }

    fn right_table_scrollbar_area(&self, area: Rect) -> Option<Rect> {
        let (table_area, list_area) = self.right_table_and_list_areas(area)?;
        if list_area.width >= table_area.width || list_area.height == 0 {
            return None;
        }

        Some(Rect::new(
            list_area.x + list_area.width,
            list_area.y,
            1,
            list_area.height,
        ))
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

    fn handle_main_list_click(&mut self, column: u16, row: u16, area: Rect) -> Option<KeyEvent> {
        let (_, list_area) = self.main_table_and_list_areas(area)?;
        if list_area.width == 0 || list_area.height == 0 {
            return None;
        }
        if column < list_area.x
            || column >= list_area.x + list_area.width
            || row < list_area.y
            || row >= list_area.y + list_area.height
        {
            return None;
        }

        let row_rel = row.saturating_sub(list_area.y) as usize;
        let target_idx = self.table_state.offset().saturating_add(row_rel);
        if target_idx >= self.entries.len() {
            return None;
        }

        self.selected_index = target_idx;
        self.table_state.select(Some(target_idx));
        if self.is_dual_panel_mode() {
            self.active_panel = DualPanelSide::Left;
        }

        let is_double_click = Self::update_list_double_click_state(
            &mut self.main_list_last_click,
            &self.current_dir,
            target_idx,
        );

        if is_double_click {
            Some(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
        } else {
            None
        }
    }

    fn handle_right_list_click(&mut self, column: u16, row: u16, area: Rect) -> Option<KeyEvent> {
        let (_, list_area) = self.right_table_and_list_areas(area)?;
        if list_area.width == 0 || list_area.height == 0 {
            return None;
        }
        if column < list_area.x
            || column >= list_area.x + list_area.width
            || row < list_area.y
            || row >= list_area.y + list_area.height
        {
            return None;
        }

        let row_rel = row.saturating_sub(list_area.y) as usize;
        let target_idx = self.right.table_state.offset().saturating_add(row_rel);
        if target_idx >= self.right.entries.len() {
            return None;
        }

        self.right.selected_index = target_idx;
        self.right.table_state.select(Some(target_idx));
        self.active_panel = DualPanelSide::Right;

        let is_double_click = Self::update_list_double_click_state(
            &mut self.right.list_last_click,
            &self.right.dir,
            target_idx,
        );

        if is_double_click {
            Some(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
        } else {
            None
        }
    }

    fn scroll_main_list_from_scrollbar_row(&mut self, area: Rect, row: u16, grab_offset: u16) {
        let Some(sb_area) = self.main_table_scrollbar_area(area) else {
            return;
        };
        let track_h = sb_area.height as usize;
        if track_h == 0 || self.entries.is_empty() {
            return;
        }
        let visible_rows = sb_area.height.max(1) as usize;
        let total_rows = self.entries.len();
        let max_scroll = total_rows.saturating_sub(visible_rows);
        if max_scroll == 0 {
            return;
        }

        let thumb_h = ((visible_rows * track_h + total_rows.saturating_sub(1)) / total_rows)
            .max(1)
            .min(track_h);
        let scroll_space = track_h.saturating_sub(thumb_h);
        if scroll_space == 0 {
            return;
        }

        let row_rel = row.saturating_sub(sb_area.y) as usize;
        let thumb_top = row_rel.saturating_sub(grab_offset as usize).min(scroll_space);
        let target_offset = (thumb_top * max_scroll + (scroll_space / 2)) / scroll_space;
        let target_index = target_offset.min(self.entries.len().saturating_sub(1));
        self.selected_index = target_index;
        self.table_state.select(Some(target_index));
        if self.is_dual_panel_mode() {
            self.active_panel = DualPanelSide::Left;
        }
    }

    fn scroll_right_list_from_scrollbar_row(&mut self, area: Rect, row: u16, grab_offset: u16) {
        let Some(sb_area) = self.right_table_scrollbar_area(area) else {
            return;
        };
        let track_h = sb_area.height as usize;
        if track_h == 0 || self.right.entries.is_empty() {
            return;
        }
        let visible_rows = sb_area.height.max(1) as usize;
        let total_rows = self.right.entries.len();
        let max_scroll = total_rows.saturating_sub(visible_rows);
        if max_scroll == 0 {
            return;
        }

        let thumb_h = ((visible_rows * track_h + total_rows.saturating_sub(1)) / total_rows)
            .max(1)
            .min(track_h);
        let scroll_space = track_h.saturating_sub(thumb_h);
        if scroll_space == 0 {
            return;
        }

        let row_rel = row.saturating_sub(sb_area.y) as usize;
        let thumb_top = row_rel.saturating_sub(grab_offset as usize).min(scroll_space);
        let target_offset = (thumb_top * max_scroll + (scroll_space / 2)) / scroll_space;
        let target_index = target_offset.min(self.right.entries.len().saturating_sub(1));
        self.right.selected_index = target_index;
        self.right.table_state.select(Some(target_index));
        self.active_panel = DualPanelSide::Right;
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent, area: Rect) -> Option<KeyEvent> {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.handle_mouse_scroll(true),
            MouseEventKind::ScrollDown => self.handle_mouse_scroll(false),
            MouseEventKind::Down(MouseButton::Right) => {
                self.file_list_scroll_dragging = false;
                if matches!(
                    self.mode,
                    AppMode::DownloadInput | AppMode::DownloadNaming
                ) {
                    self.paste_clipboard_at_input_cursor();
                    return None;
                }
                if matches!(self.mode, AppMode::Browsing | AppMode::PathEditing) {
                    return Some(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.is_dual_panel_mode() {
                    if let Some((left_frame, right_frame)) = self.dual_panel_frame_areas(area) {
                        let in_left = mouse.column >= left_frame.x
                            && mouse.column < left_frame.x + left_frame.width
                            && mouse.row >= left_frame.y
                            && mouse.row < left_frame.y + left_frame.height;
                        let in_right = mouse.column >= right_frame.x
                            && mouse.column < right_frame.x + right_frame.width
                            && mouse.row >= right_frame.y
                            && mouse.row < right_frame.y + right_frame.height;
                        if in_left {
                            self.active_panel = DualPanelSide::Left;
                        } else if in_right {
                            self.active_panel = DualPanelSide::Right;
                        }
                    }

                    if let Some(sb_area) = self.right_table_scrollbar_area(area) {
                        if mouse.column >= sb_area.x
                            && mouse.column < sb_area.x + sb_area.width
                            && mouse.row >= sb_area.y
                            && mouse.row < sb_area.y + sb_area.height
                        {
                            let total_rows = self.right.entries.len();
                            if let Some(grab_offset) = Self::scrollbar_grab_offset_for_row(
                                sb_area,
                                total_rows,
                                self.right.table_state.offset(),
                                mouse.row,
                            ) {
                                self.right.list_scroll_grab_offset = grab_offset;
                                self.right.list_scroll_dragging = true;
                                self.scroll_right_list_from_scrollbar_row(
                                    area,
                                    mouse.row,
                                    self.right.list_scroll_grab_offset,
                                );
                                return None;
                            }
                        }
                    }
                }

                if let Some(sb_area) = self.main_table_scrollbar_area(area) {
                    if mouse.column >= sb_area.x
                        && mouse.column < sb_area.x + sb_area.width
                        && mouse.row >= sb_area.y
                        && mouse.row < sb_area.y + sb_area.height
                    {
                        let total_rows = self.entries.len();
                        if let Some(grab_offset) = Self::scrollbar_grab_offset_for_row(
                            sb_area,
                            total_rows,
                            self.table_state.offset(),
                            mouse.row,
                        ) {
                            self.file_list_scroll_grab_offset = grab_offset;
                            self.file_list_scroll_dragging = true;
                            self.scroll_main_list_from_scrollbar_row(
                                area,
                                mouse.row,
                                self.file_list_scroll_grab_offset,
                            );
                            return None;
                        }
                    }
                }
                self.file_list_scroll_dragging = false;
                self.right.list_scroll_dragging = false;
                if self.handle_preview_pane_tab_click(mouse.column, mouse.row, area) {
                    return None;
                }
                if let Some(key) = self.handle_main_list_click(mouse.column, mouse.row, area) {
                    return Some(key);
                }
                if let Some(key) = self.handle_right_list_click(mouse.column, mouse.row, area) {
                    return Some(key);
                }
                if self.handle_tab_close_click(mouse.column, mouse.row, area) {
                    return None;
                }
                if self.handle_tab_click(mouse.column, mouse.row, area) {
                    return None;
                }
                if let Some(key) = self.clickable_key_from_tabbed_row(mouse.column, mouse.row, area) {
                    return Some(key);
                }
                let _ = self.handle_confirm_delete_click(mouse.column, mouse.row, area);
                if self.handle_confirm_integration_install_click(mouse.column, mouse.row, area) {
                    return None;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.file_list_scroll_dragging {
                    self.scroll_main_list_from_scrollbar_row(
                        area,
                        mouse.row,
                        self.file_list_scroll_grab_offset,
                    );
                    return None;
                }
                if self.right.list_scroll_dragging {
                    self.scroll_right_list_from_scrollbar_row(
                        area,
                        mouse.row,
                        self.right.list_scroll_grab_offset,
                    );
                    return None;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.file_list_scroll_dragging = false;
                self.right.list_scroll_dragging = false;
            }
            _ => {}
        }

        None
    }

    fn load_bookmarks() -> Vec<(usize, Option<PathBuf>)> {
        (0..=9).map(|i| {
            let path = env::var(format!("SB_BOOKMARK_{}", i))
                .ok()
                .map(PathBuf::from)
                .filter(|p| p.is_dir());
            (i, path)
        }).collect()
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
        if !list_args.include_hidden && list_args.tree_depth.is_none() {
            if let Some(path) = list_args.path {
                let target = PathBuf::from(path);
                if target.is_file() {
                    return App::open_path_in_view_mode(&target, true);
                }
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
    if args.len() == 1 && !args[0].starts_with('-') {
        if let Ok(target) = PathBuf::from(&args[0]).canonicalize() {
            if target.is_dir() {
                return ui::cli::list_current_directory(false, false, None, Some(&args[0]));
            }
        }
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