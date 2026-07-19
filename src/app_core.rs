//! Core `App` methods (navigation, entries, input, confirm dialogs, bookmarks).
//! Extracted from main.rs to keep the entry-point file navigable.
use crate::*;

impl App {
    pub(crate) fn open_path_in_editor_cli(path: &PathBuf) -> io::Result<()> {
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

    pub(crate) fn new() -> io::Result<Self> {
        let current_dir = app_init::init_current_dir()?;
        let internal_search_content_limits = Self::internal_search_content_limits();
        let mut app = Self {
            left: PanelState {
                dir: current_dir,
                entries: Vec::new(),
                entry_render_cache: Vec::new(),
                list_aggregates: ListAggregates::default(),
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
                selected_total_size: AsyncJobState::default(),
                selected_total_size_pending: false,
                selected_total_size_bytes: None,
                selected_total_size_items: 0,
            },
            directory_selection: HashMap::new(),
            archive_mounts: Vec::new(),
            mode: AppMode::Browsing,
            clipboard: Vec::new(),
            folder_filter_visible: false,
            input_buffer: String::new(),
            input_cursor: 0,
            status_message: String::new(),
            right_status_message: String::new(),
            page_size: 20,
            remote: RemoteState::default(),
            copy: CopyOperation::default(),
            transfer: TransferState::default(),
            archive: ArchiveOperation::default(),
            nerd_font_active: env::var("NERD_FONT_ACTIVE").map(|v| v == "1").unwrap_or(false),
            filename_color_mode: FilenameColorMode::Full,
            os_icon: ui::icons::os_nerd_icon().map(|(g, _)| {
                (g, ui::theme::theme_spec(ui::theme::ThemeId::original()).icon_os)
            }),
            no_color: env_flag_true(&["NO_COLOR"]),
            show_icons: env::var("TERMINAL_ICONS").map(|v| v != "0").unwrap_or(true),
            bookmarks: BookmarkUiState {
                cache: Self::load_bookmarks(),
                ..Default::default()
            },
            integration: IntegrationUiState::default(),
            confirm_delete: ConfirmDeleteState::default(),
            help: HelpState::default(),
            confirm_integration_install_button_focus: 0,
            git: GitInfoState::default(),
            disable_clock: false,
            size: SizeState::default(),
            tree: TreeState::default(),
            sort_menu_selected: 0,
            panel_tab: 0,
            active_theme: ui::theme::ThemeId::original(),
            themes: ThemePanelState::default(),
            settings_selected: 0,
            keymap: util::keymap::KeyMap::default(),
            plugins: plugin::runtime::PluginRuntime::empty(),
            shortcuts_panel: ShortcutsPanelState::default(),
            plugins_panel: PluginsPanelState::default(),
            ai: AiState {
                provider: "groq".to_string(),
                ..Default::default()
            },
            organize: OrganizeState::default(),
            search: SearchState {
                candidates: Vec::new(),
                candidate_symlinks: HashSet::new(),
                results: Vec::new(),
                selected: 0,
                scope: InternalSearchScope::Filename,
                candidates_rx: None,
                candidates_scan_id: 0,
                candidates_pending: false,
                candidates_truncated: false,
                content_rx: None,
                content_request_id: 0,
                content_pending: false,
                content_limit_note: None,
                content_limits: internal_search_content_limits,
                limits_menu_open: false,
                limits_selected: 0,
                regex_mode: false,
                regex: None,
                regex_error: None,
            },
            notes: NotesState::default(),
            meta: MetaState {
                group_width: 1,
                owner_width: 1,
            },
            header_clock: HeaderClockState::default(),
            needs_redraw: true,
            db_preview: DbPreviewState {
                row_limit: 8,
                ..Default::default()
            },
            view_mode: ViewMode::Normal,
            preview: PreviewState::default(),
            pane_video: None,
            pane_video_want_since: None,
            footer_shortcut_zones: Vec::new(),
            active_panel: DualPanelSide::Left,
            right: PanelState {
                dir: PathBuf::new(),
                entries: Vec::new(),
                entry_render_cache: Vec::new(),
                list_aggregates: ListAggregates::default(),
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
                selected_total_size: AsyncJobState::default(),
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
        // Persisted "disable clock" choice: show the disk pill instead of the clock.
        if let Some(dc) = persist.disable_clock {
            app.disable_clock = dc;
        }
        // Persisted filename-color mode; applied before set_active_theme so the
        // first render-cache build uses it.
        app.filename_color_mode = persist.filename_color_mode;
        // Restore persisted AI commit-message settings (provider/model/key).
        app.ai.provider = persist.ai_provider.clone();
        app.ai.model = persist.ai_model.clone();
        app.ai.api_keys = persist.ai_api_keys.clone();
        // The live buffer shows the active provider's key.
        app.ai.api_key = app
            .ai.api_keys
            .get(&app.ai.provider)
            .cloned()
            .unwrap_or_default();
        app.ai.auto_commit = persist.ai_auto_commit;
        // Restore custom keyboard shortcuts.
        app.keymap = util::keymap::KeyMap::from_overrides(&persist.shortcuts);
        app.set_active_theme(ui::theme::theme_by_name(&persist.current_theme));
        for key in &persist.disabled_integrations {
            app.integration.overrides.insert(key.clone(), false);
        }
        // Restore persisted folder-size calculation toggle (kicks off the scan).
        if persist.folder_size_enabled {
            app.set_folder_size_enabled(true);
        }
        // Load Lua plugins and fire their on_start hooks. Load/hook errors
        // are recorded per-plugin and surfaced in the Plugins panel.
        let sources = plugin::discovery::discover(&plugin::discovery::plugins_dir());
        if !sources.is_empty() {
            app.plugins = plugin::runtime::PluginRuntime::init(
                sources,
                &persist.disabled_plugins,
                &persist.plugin_bindings,
            );
            if app.plugins.wants_hook(plugin::runtime::Hook::Start) {
                let ctx = app.plugin_ctx();
                let effects = app.plugins.run_hook(plugin::runtime::Hook::Start, &ctx);
                app.apply_plugin_effects(effects, false);
            }
        }
        Ok(app)
    }

    /// Refresh the cached header clock string. Returns `true` when the displayed
    /// minute changed (so the event loop knows it must repaint).
    pub(crate) fn refresh_header_clock_if_needed(&mut self) -> bool {
        let now = Local::now();
        let minute_key = now.timestamp().div_euclid(60);
        if self.header_clock.minute_key == Some(minute_key) {
            return false;
        }
        self.header_clock.minute_key = Some(minute_key);
        self.header_clock.text = now.format("%Y-%m-%d %H:%M").to_string();
        true
    }

    /// Whether any background job still has a live channel that a future `pump_*`
    /// call could deliver results on. The event loop uses this to decide whether
    /// an idle (event-less) iteration still needs to repaint.
    ///
    /// INVARIANT: every async `Receiver`/`AsyncJobState` field on `App`/`PanelState`
    /// must be listed here. Add new background sources to this list or their
    /// results may not appear until the next user input.
    pub(crate) fn has_active_async_work(&self) -> bool {
        self.copy.rx.is_some()
            || self.copy.total_rx.is_some()
            || self.archive.rx.is_some()
            || self.search.candidates_rx.is_some()
            || self.search.content_rx.is_some()
            || self.transfer.download_rx.is_some()
            || self.git.info_rx.is_some()
            || self.size.folder_size.rx.is_some()
            || self.size.current_dir_total_size.rx.is_some()
            || self.size.recursive_mtime.rx.is_some()
            || self.left.selected_total_size.rx.is_some()
            || self.right.selected_total_size.rx.is_some()
            || self.notes.rx.is_some()
            || self.notes.right_rx.is_some()
            || self.preview.rx.is_some()
            || self.ai.commit_rx.is_some()
            || self.ai.key_check_rx.is_some()
            || self.ai.key_edit_at.is_some()
            || self.organize.rx.is_some()
            || self.plugins.has_pending_spawns()
    }

    pub(crate) fn search_spans_with_ranges(
        text: &str,
        ranges: &[(usize, usize)],
        base_style: Style,
        match_style: Style,
    ) -> Vec<Span<'static>> {
        ui::search::search_spans_with_ranges(text, ranges, base_style, match_style)
    }

    pub(crate) fn refresh_entries_or_status(&mut self) -> bool {
        match self.refresh_entries() {
            Ok(()) => {
                if self.copy.rx.is_none() && self.archive.rx.is_none() {
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

    pub(crate) fn try_enter_dir(&mut self, target: PathBuf) {
        let previous_dir = self.left.dir.clone();
        let previous_filter = self.left.folder_filter.clone();
        let changed_dir = target != previous_dir;
        self.remember_current_selection();
        self.left.dir = target;
        if changed_dir {
            self.left.folder_filter = None;
            self.folder_filter_visible = false;
        }
        if !self.refresh_entries_or_status() {
            self.left.dir = previous_dir;
            self.left.folder_filter = previous_filter;
        } else {
            self.restore_selection_for_current_dir();
            self.request_git_info_for_current_dir_once();
        }
    }

    pub(crate) fn active_panel_dir(&self) -> PathBuf {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.dir.clone()
        } else {
            self.left.dir.clone()
        }
    }

    pub(crate) fn active_selected_entry_path(&self) -> Option<PathBuf> {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.entries.get(self.right.selected_index).map(|e| e.path())
        } else {
            self.left.entries.get(self.left.selected_index).map(|e| e.path())
        }
    }

    pub(crate) fn active_entries_empty(&self) -> bool {
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.entries.is_empty()
        } else {
            self.left.entries.is_empty()
        }
    }

    pub(crate) fn try_enter_dir_on_active_panel(&mut self, target: PathBuf) {
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
    pub(crate) fn folder_filter_on_left(&self) -> bool {
        self.folder_filter_visible
            && !(self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right)
    }

    /// Whether the folder-filter box currently applies to the right panel.
    pub(crate) fn folder_filter_on_right(&self) -> bool {
        self.folder_filter_visible
            && self.is_dual_panel_mode()
            && self.active_panel == DualPanelSide::Right
    }

    /// Open the folder-filter box on the active panel, seeding it with the
    /// current filter pattern (if any) so it can be edited in place.
    pub(crate) fn begin_folder_filter(&mut self) {
        let current = if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            self.right.folder_filter.as_ref().map(|f| f.pattern.clone())
        } else {
            self.left.folder_filter.as_ref().map(|f| f.pattern.clone())
        }
        .unwrap_or_default();
        self.folder_filter_visible = true;
        self.begin_input_edit(AppMode::FolderFilter, current);
    }

    /// Re-derive and apply the folder filter from the current input buffer to
    /// the active panel, refreshing its listing live.
    pub(crate) fn apply_folder_filter_live(&mut self) {
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
            self.left.folder_filter = new_filter;
            self.refresh_entries_or_status();
        }
    }

    /// Hide the folder-filter box and clear the filter on the active panel.
    pub(crate) fn clear_folder_filter(&mut self) {
        self.folder_filter_visible = false;
        self.clear_input_edit();
        self.mode = AppMode::Browsing;
        if self.is_dual_panel_mode() && self.active_panel == DualPanelSide::Right {
            if self.right.folder_filter.take().is_some()
                && self.refresh_right_panel_entries().is_err()
            {
                self.set_status("refresh failed");
            }
        } else if self.left.folder_filter.take().is_some() {
            self.refresh_entries_or_status();
        }
    }




    pub(crate) fn create_temp_selection_path(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        env::temp_dir().join(format!("{}_{}_{}.txt", prefix, std::process::id(), stamp))
    }

    pub(crate) fn remote_mount_for_path(&self, path: &PathBuf) -> Option<&SshMount> {
        self.remote.ssh_mounts
            .iter()
            .filter(|mount| path == &mount.mount_path || path.starts_with(&mount.mount_path))
            .max_by_key(|mount| mount.mount_path.components().count())
    }

    pub(crate) fn display_path_for(&self, path: &PathBuf) -> String {
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

    pub(crate) fn remember_current_selection(&mut self) {
        self.directory_selection
            .insert(self.left.dir.clone(), self.left.selected_index);
    }

    pub(crate) fn restore_selection_for_current_dir(&mut self) {
        if self.left.entries.is_empty() {
            self.left.selected_index = 0;
            self.left.table_state.select(None);
            return;
        }

        let index = self
            .directory_selection
            .get(&self.left.dir)
            .copied()
            .unwrap_or(0)
            .min(self.left.entries.len() - 1);
        self.left.selected_index = index;
        self.left.table_state.select(Some(index));
    }

    pub(crate) fn select_entry_named(&mut self, name: &str) {
        if let Some(index) = self.left
            .entries
            .iter()
            .position(|entry| entry.file_name().to_string_lossy() == name)
        {
            self.left.selected_index = index;
            self.left.table_state.select(Some(index));
        }
    }

    pub(crate) fn select_right_entry_named(&mut self, name: &str) {
        if let Some(index) = self
            .right.entries
            .iter()
            .position(|entry| entry.file_name().to_string_lossy() == name)
        {
            self.right.selected_index = index;
            self.right.table_state.select(Some(index));
        }
    }

    pub(crate) fn try_enter_parent_dir(&mut self) {
        let child_name = self.left
            .dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());

        if let Some(parent) = self.left.dir.parent() {
            self.try_enter_dir(parent.to_path_buf());
            if let Some(name) = child_name {
                self.select_entry_named(&name);
            }
        }
    }

    pub(crate) fn resolve_input_path(&self, raw: &str) -> PathBuf {
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
            self.left.dir.join(candidate)
        }
    }

    pub(crate) fn apply_path_input(&mut self) {
        let raw_input = self.input_buffer.trim().to_string();
        let target = self.resolve_input_path(&raw_input);
        if target.is_dir() {
            // No-op: same directory, no filter to clear — skip refresh to avoid timestamp flicker
            if target == self.left.dir && self.left.folder_filter.is_none() {
                self.mode = AppMode::Browsing;
                self.clear_input_edit();
                return;
            }
            self.left.folder_filter = None;
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
        self.left.folder_filter = Some(filter);
        self.refresh_entries_or_status();
        self.mode = AppMode::Browsing;
        self.clear_input_edit();
    }

    pub(crate) fn input_cursor_line_col(&self) -> (usize, usize) {
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

    pub(crate) fn active_input_line_text(&self) -> String {
        let (line_idx, _) = self.input_cursor_line_col();
        self.input_buffer
            .split('\n')
            .nth(line_idx)
            .unwrap_or_default()
            .to_string()
    }

    pub(crate) fn create_entries_from_input(&mut self, default_is_dir: bool) {
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
        self.sync_inactive_panel_if_same_dir();

        if failed == 0 {
            self.set_status(format!("created {} item(s)", created.len()));
        } else {
            self.set_status(format!("created {} item(s), {} failed", created.len(), failed));
        }
    }

    pub(crate) fn paste_clipboard_at_input_cursor(&mut self) {
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

    pub(crate) fn refresh_entries(&mut self) -> io::Result<()> {
        let folder_size_cache = if self.size.folder_size_enabled {
            Some(&self.size.folder_size_cache)
        } else {
            None
        };
        let mut tree_row_prefixes = Vec::new();
        let mut entries: Vec<_> = if !self.tree.expansion_levels.is_empty() {
            let rows = ui::tree::collect_tree_rows_with_expansions(
                &self.left.dir,
                self.left.show_hidden,
                self.left.sort_mode,
                folder_size_cache,
                &self.tree.expansion_levels,
            )?;
            tree_row_prefixes = rows.iter().map(|row| row.prefix.clone()).collect();
            rows.into_iter().map(|row| row.entry).collect()
        } else {
            let mut direct_entries: Vec<_> = fs::read_dir(&self.left.dir)?
                .filter_map(|res| res.ok())
                .filter(|e| self.left.show_hidden || !crate::util::classify::is_hidden_entry(e))
                .collect();
            Self::sort_entries_by_mode(&mut direct_entries, self.left.sort_mode, folder_size_cache);
            direct_entries
        };
        if let Some(filter) = self.left.folder_filter.as_ref() {
            let filter_regex = Self::build_path_filter_regex(filter)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
            if !self.tree.expansion_levels.is_empty() {
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
        self.left.entries = entries;
        self.left.tree_row_prefixes = if !self.tree.expansion_levels.is_empty() {
            tree_row_prefixes
        } else {
            vec![String::new(); self.left.entries.len()]
        };
        let config = EntryRenderConfig {
            nerd_font_active: self.nerd_font_active,
            show_icons: self.show_icons,
            theme_id: self.active_theme,
            filename_color_mode: self.filename_color_mode,
        };
        let uid_cache = App::build_uid_cache(&self.left.entries);
        let gid_cache = App::build_gid_cache(&self.left.entries);
            self.left.entry_render_cache = self.left.entries.iter()
            .map(|entry| App::build_entry_render_cache(entry, config, &uid_cache, &gid_cache))
            .collect();
        self.apply_cached_folder_size_columns();
        self.refresh_meta_identity_widths();
        self.refresh_current_dir_free_space();
        self.size.folder_size.next_scan_id();
        self.size.folder_size.clear_rx();
        self.size.recursive_mtime.clear_rx();
        self.clear_current_dir_total_size_state();
        self.clear_selected_total_size_state();
        self.left.marked_indices.clear();
        
        if self.left.entries.is_empty() {
            self.left.selected_index = 0;
            self.left.table_state.select(None);
        } else {
            self.left.selected_index = self.left.selected_index.min(self.left.entries.len() - 1);
            self.left.table_state.select(Some(self.left.selected_index));
        }

        if self.size.folder_size_enabled {
            self.start_folder_size_scan();
            self.start_current_dir_total_size_scan();
        }
        self.start_recursive_mtime_scan();
        self.request_notes_for_current_dir_once();
        self.request_notes_for_right_panel_once();
        Ok(())
    }

    pub(crate) fn request_notes_for_right_panel_once(&mut self) {
        // No right-panel directory yet (e.g. before dual-panel mode is entered).
        if self.right.dir.as_os_str().is_empty() {
            return;
        }
        if self.notes.right_loaded_for.as_ref().map(|p| p == &self.right.dir).unwrap_or(false) {
            return;
        }
        let dir = self.right.dir.clone();
        self.notes.right_by_name.clear();
        self.notes.right_rx = Some(util::background::spawn_worker(move |tx| {
            let notes = App::load_notes_map_for_dir(&dir);
            let _ = tx.send(NotesLoadMsg::Finished(0, dir, notes));
        }));
    }

    pub(crate) fn pump_right_notes_progress(&mut self) {
        let Some(rx) = self.notes.right_rx.take() else {
            return;
        };
        let mut keep_rx = true;
        loop {
            match rx.try_recv() {
                Ok(NotesLoadMsg::Finished(_, path, notes)) => {
                    if path == self.right.dir {
                        self.notes.right_by_name = notes;
                        self.notes.right_loaded_for = Some(path);
                        keep_rx = false;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => { keep_rx = false; break; }
            }
        }
        if keep_rx {
            self.notes.right_rx = Some(rx);
        }
    }

    pub(crate) fn delete_targets(&self) -> Vec<PathBuf> {
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
        } else if !self.left.marked_indices.is_empty() {
            self.left.entries
                .iter()
                .enumerate()
                .filter(|(i, _)| self.left.marked_indices.contains(i))
                .map(|(_, e)| e.path())
                .collect()
        } else {
            self.left.entries
                .get(self.left.selected_index)
                .map(|e| e.path())
                .into_iter()
                .collect()
        }
    }

    pub(crate) fn begin_confirm_delete(&mut self) {
        // Stat the targets once here; the dialog is redrawn every frame and
        // must not touch the filesystem while rendering.
        self.confirm_delete.targets = self
            .delete_targets()
            .into_iter()
            .map(|path| ui::dialogs::DeleteTarget {
                is_dir: path.is_dir(),
                is_symlink: crate::util::classify::is_symlink(&path),
                path,
            })
            .collect();
        self.confirm_delete.scroll_offset = 0;
        self.confirm_delete.max_offset = 0;
        self.confirm_delete.button_focus = 0;
        self.mode = AppMode::ConfirmDelete;
    }

    pub(crate) fn confirm_delete_selected_targets(&mut self) {
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
        self.sync_inactive_panel_if_same_dir();
        // Surface delete failures (e.g. permission denied) instead of silently
        // leaving the item in place; this status takes priority over refresh's.
        if let Some(err) = last_err {
            self.set_status(format!("delete failed for {} item(s): {}", failed, err));
        }
    }

    pub(crate) fn cancel_integration_install_prompt(&mut self) {
        self.confirm_integration_install_button_focus = 1;
        self.mode = AppMode::Integrations;
        self.clear_integration_install_prompt();
        self.set_status("integration install cancelled");
    }

    pub(crate) fn handle_ok_cancel_focus_key(key: KeyCode, focus: &mut u8, allow_hl_tab: bool) -> bool {
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

    pub(crate) fn handle_confirm_integration_install_key(&mut self, key: KeyEvent) -> io::Result<bool> {
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

    pub(crate) fn handle_confirm_delete_key(&mut self, key: KeyEvent) {
        if Self::handle_ok_cancel_focus_key(key.code, &mut self.confirm_delete.button_focus, false)
        {
            return;
        }

        match key.code {
            KeyCode::Up => {
                self.confirm_delete.scroll_offset = self.confirm_delete.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down => {
                self.confirm_delete.scroll_offset =
                    (self.confirm_delete.scroll_offset + 1).min(self.confirm_delete.max_offset);
            }
            KeyCode::PageUp => {
                self.confirm_delete.scroll_offset = self.confirm_delete.scroll_offset.saturating_sub(8);
            }
            KeyCode::PageDown => {
                self.confirm_delete.scroll_offset =
                    (self.confirm_delete.scroll_offset + 8).min(self.confirm_delete.max_offset);
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                if key.code == KeyCode::Enter && self.confirm_delete.button_focus == 1 {
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

    pub(crate) fn handle_organize_key(&mut self, key: KeyEvent) {
        if Self::handle_ok_cancel_focus_key(key.code, &mut self.organize.button_focus, false) {
            return;
        }

        match key.code {
            KeyCode::Up => {
                self.organize.scroll_offset = self.organize.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down => {
                self.organize.scroll_offset =
                    (self.organize.scroll_offset + 1).min(self.organize.max_offset);
            }
            KeyCode::PageUp => {
                self.organize.scroll_offset = self.organize.scroll_offset.saturating_sub(8);
            }
            KeyCode::PageDown => {
                self.organize.scroll_offset =
                    (self.organize.scroll_offset + 8).min(self.organize.max_offset);
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                if key.code == KeyCode::Enter && self.organize.button_focus == 1 {
                    self.cancel_organize();
                } else if self.organize.plan.is_some() {
                    self.apply_organize_plan();
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.cancel_organize();
            }
            _ => {}
        }
    }

    pub(crate) fn handle_confirm_extract_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') => {
                self.mode = AppMode::Browsing;
                self.extract_archives_confirmed();
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.archive.extract_targets.clear();
                self.mode = AppMode::Browsing;
                self.set_status("extract cancelled");
            }
            _ => {}
        }
    }









    pub(crate) fn panel_tab_bar_line(active: u8, theme_id: ui::theme::ThemeId, nerd_font: bool, avail_width: u16) -> Line<'static> {
        ui::panels::panel_tab_bar_line(active, theme_id, nerd_font, avail_width)
    }

    pub(crate) fn dual_panel_frame_areas(&self, area: Rect) -> Option<(Rect, Rect)> {
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

    pub(crate) fn update_list_double_click_state(
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

    pub(crate) fn scrollbar_grab_offset_for_row(
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

        let (thumb_y, thumb_h) =
            ui::scrollbar::scrollbar_thumb(total_rows, visible_rows, offset, track_h);

        let row_rel = row.saturating_sub(sb_area.y) as usize;
        let in_thumb = row_rel >= thumb_y && row_rel < thumb_y + thumb_h;
        Some(if in_thumb {
            (row_rel.saturating_sub(thumb_y)) as u16
        } else {
            (thumb_h / 2) as u16
        })
    }

    /// Cached bookmark slots; cheap to call from render and input paths.
    pub(crate) fn bookmarks(&self) -> &[(usize, Option<PathBuf>)] {
        &self.bookmarks.cache
    }

    /// Re-read the bookmark slots from config/env. Called when the Bookmarks
    /// overlay opens and after any bookmark write.
    pub(crate) fn refresh_bookmarks_cache(&mut self) {
        self.bookmarks.cache = Self::load_bookmarks();
    }

    pub(crate) fn load_bookmarks() -> Vec<(usize, Option<PathBuf>)> {
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

    pub(crate) fn delete_bookmark(&mut self, idx: usize) {
        let from_env = env::var(format!("SB_BOOKMARK_{}", idx)).is_ok();
        let result = crate::util::config::SbPersistConfig::update(|cfg| {
            if from_env {
                cfg.bookmarks.insert(idx as u8, "<deleted>".to_string());
            } else {
                cfg.bookmarks.remove(&(idx as u8));
            }
        });
        if let Err(e) = result {
            self.set_status(format!("failed to save bookmarks: {}", e));
        }
        self.refresh_bookmarks_cache();
    }

    pub(crate) fn handle_confirm_delete_bookmark_key(&mut self, key: KeyEvent) {
        if Self::handle_ok_cancel_focus_key(key.code, &mut self.confirm_delete.bookmark_button_focus, false) {
            return;
        }
        match key.code {
            KeyCode::Char('y') => {
                self.confirm_delete.bookmark_button_focus = 0;
                self.delete_bookmark(self.bookmarks.delete_idx);
                self.mode = AppMode::Bookmarks;
            }
            KeyCode::Enter => {
                if self.confirm_delete.bookmark_button_focus == 0 {
                    self.delete_bookmark(self.bookmarks.delete_idx);
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
