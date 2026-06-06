# sb (Shell Buddy / sb) — Copilot Instructions

A terminal-based file manager TUI written in Rust.

## Code Layout (Modular + Layered)

The codebase eliminates duplication via a **foundation layer** (`util`), a **UI abstraction layer** (`ui`), and **focused app modules** (`app_*.rs`). Core orchestration remains in `main.rs`.

### Source Tree Ownership

#### `src/util/` — Foundation Helpers (Shared, No Duplication)

Reusable logic imported by all modules. **No inter-util dependencies** except `config` (singleton).

- `config.rs` — **Centralized app config** parsed once at startup
  - `struct AppConfig`: `nerd_font_active`, `no_color`, `show_icons`, `editor`, `preview_line_limit`, `dir_list_limit`, `binary_threshold`, `search_candidate_limit`, `db_preview_row_limit`
  - Call `AppConfig::from_env()` once in `App::new()` and store as `app.config`
  - **Never** scatter `env::var(...)` calls outside of this module
- `cleanup.rs` — Safe file cleanup returning `io::Result` instead of silent `let _`
  - `fn safe_cleanup_path(path)` → removes file or dir, `NotFound` is OK
  - `fn safe_unlink_path(path)` → same but handles both files and dirs
  - Replaces ~20 `let _ = fs::remove_file/dir()` patterns
- `classify.rs` — Unified path metadata classification
  - `struct PathClass { is_dir, is_symlink, size }`
  - `fn classify_path(path) -> io::Result<PathClass>`
  - `fn get_metadata(path) -> io::Result<Metadata>` (follows symlinks)
  - Replaces ~5 duplicated `fs::symlink_metadata() + is_dir() + is_symlink()` patterns
- `command.rs` — Command execution builder pattern
  - `CommandBuilder::git_command(cwd, args)` → `io::Result<Output>`
  - `CommandBuilder::unmount_archive(mount_point)` → `io::Result<()>`
  - `CommandBuilder::preview_command(tool, path, args)` → `io::Result<Output>`
  - `CommandBuilder::archive_command(tool, args, cwd)` → `io::Result<Output>`
  - `CommandBuilder::tool_available(tool)` → `bool`
  - Replaces ~20 scattered `Command::new()` patterns
- `background.rs` — Generic background task polling
  - `struct BackgroundTask<T>` wrapping `std::sync::mpsc::Receiver<T>`
  - `BackgroundTask::new(rx)`, `BackgroundTask::inactive()`
  - `.try_recv() -> Result<T, String>`
  - Single `pump_background_tasks()` in main loop handles all background channels
- `format.rs` — ✓ Existing: `fn format_size()`, `fn format_eta()`

#### `src/ui/` — Rendering & Modal Abstraction

UI component builders, rendering helpers, and modal abstraction. All use `ui/palette.rs` for colors.

- `palette.rs` — **Color theme constants** (single source of truth)
  - `Palette::TEXT_BRIGHT`, `TEXT_DIM`, `ACCENT_PRIMARY`, `SUCCESS`, `ERROR`, `WARNING`, etc.
  - Helper methods: `Palette::default_text()`, `dim_text()`, `highlight()`, `error()`, `success()`, `warning()`, `selection_bg()`, `marked_bg()`
  - **Never** hard-code `Color::Rgb(...)` outside this file
- `spans.rs` — **Span construction helpers**
  - `fn titled_span(title, content, sep) -> Vec<Span<'static>>`
  - `fn status_span(msg, icon, StatusType) -> Span<'static>`
  - `fn entry_type_span(name, EntryType) -> Span<'static>`
  - `fn dim_span/highlight_span/error_span/success_span/warning_span(text)`
  - Enums: `StatusType`, `EntryType` (File, Directory, Symlink, Archive, Executable, Modified, Added, Deleted)
  - Replaces ~40 scattered `Span::styled()` calls
- `modal.rs` — **Modal dialog abstraction**
  - `trait Modal { handle_event(KeyEvent) -> ModalResult, render(Frame, Rect), title(), is_scrollable(), scroll_down/up() }`
  - `enum ModalResult { Open, Cancelled, Confirmed, Action(u8) }`
  - `struct HelpModal`, `struct ConfirmModal`, `struct BookmarkModal`
  - Centralizes help, confirm, bookmark dialog patterns
- `panels.rs` — ✓ Existing; panel/tab bar builders — **should use** `ui/palette.rs`
- `cli.rs` — ✓ Existing; list-mode rendering — **should use** `util/config.rs`
- `icons.rs` — ✓ Existing; file-type icon mapping
- `search.rs` — ✓ Existing; search highlighting spans — **should use** `ui/palette.rs`
- `status.rs` — ✓ Existing; status footer decoration — **should use** `ui/palette.rs`, `ui/spans.rs`
- `dialogs.rs` — ✓ Existing; dialog rendering helpers
- `list_render.rs` — ✓ Existing; list rendering helpers
- `list_temperature.rs` — ✓ Existing; list temperature indicators
- `tree.rs` — ✓ Existing; tree-mode rendering

#### `src/app_*.rs` — App State & Business Logic (Focused Modules)

Each module owns one concern. All use `util/` and `ui/` helpers instead of duplicating logic.

- `app_init.rs` — **Startup initialization helpers**
  - `fn init_config() -> AppConfig` — parse config from env
  - `fn init_current_dir() -> io::Result<PathBuf>` — get starting directory
- `app_modes.rs` — **AppMode state machine**
  - `enum AppModeTransition`, `enum TransitionResult`
  - `struct ModeController` — validates mode transitions centrally
  - Prevents scattered `self.mode = AppMode::*` assignments
- `app_entry_iter.rs` — **Entry filtering/iteration helpers**
  - `struct FilteredEntryIter { indices: Vec<usize> }`
  - `.count()`, `.total_size(size_fn)` — abstracts index-heavy loops
- `app_input.rs` — ✓ Existing; input buffer & cursor editing
- `app_meta.rs` — ✓ Existing; permissions/owner/group, UID/GID caches
- `app_render_cache.rs` — ✓ Existing; entry row rendering — **should use** `util/classify.rs`, `ui/palette.rs`, `ui/spans.rs`
- `app_search.rs` — ✓ Existing; search logic — **should use** `util/config.rs` for limits
- `app_files.rs` — ✓ Existing; file type classification — **should use** `util/config.rs` for thresholds
- `app_preview.rs` — Preview pane loading and content building (text/image/binary)
- `app_sizes.rs` — ✓ Existing; folder walking — **should use** `util/classify.rs`
- `app_git.rs` — ✓ Existing; git status and commit/tag workflows — **should use** `util/command.rs`
- `app_archive.rs` — ✓ Existing; archive mount/preview and create/extract job pipeline — **should use** `util/command.rs`, `util/cleanup.rs`
- `app_remote.rs` — SSH/rclone/local mount discovery and mount lifecycle helpers
- `app_shell.rs` — Split-shell helpers and external shell command runner flows
- `app_transfer.rs` — Clipboard transfer pipeline, copy/move progress jobs, and clipboard-backend helpers
- `app_mouse.rs` — Mouse event handlers, scrollbar click/drag, tab click/close, list double-click detection
- `app_images.rs` — ✓ Existing; image caching
- `app_notes.rs` — Directory note parsing, async loading, and note edit/save flows
- `app_sqlite.rs` — SQLite table discovery and DB preview row rendering logic

#### `src/integration/` — External Tool Integration

- `app.rs` — Integration control flow — **should use** `util/config.rs`, `util/command.rs`
- `catalog.rs` — Integration definitions & package mappings
- `probe.rs` — Runtime tool detection — **should use** `util/command.rs`
- `rows.rs` — Integration row rendering — **should use** `ui/palette.rs`

#### `src/main.rs` — Event Loop & Orchestration

- **Owns**: crossterm raw mode, ratatui render loop, event pump, top-level dispatch, exit cleanup
- **Delegates to**: `app_init` (startup), `app_modes` (state machine), `ui/modal` (dialog rendering), `util/*` (helpers)
- **Target size**: ~400 lines of pure orchestration (currently ~1400 due to in-progress extraction)

## Build & Test

```bash
cargo build            # debug build
cargo build --release  # optimized (size: opt-level=z, lto, strip)
cargo run              # run directly
cargo test             # run tests
```

## Architecture

```
util/config.rs ← (singleton, foundation for all modules)
util/cleanup.rs, classify.rs, command.rs, background.rs, format.rs

ui/palette.rs ← (theme, required by all UI modules)
ui/spans.rs ← palette
ui/modal.rs ← palette
ui/panels.rs, cli.rs, search.rs, status.rs ← palette, spans

app_init.rs ← util/config
app_modes.rs ← util/config, ui/modal
app_entry_iter.rs ← util/classify
app_render_cache.rs ← util/classify, ui/palette, ui/spans
app_search.rs, app_files.rs ← util/config
app_preview.rs ← util/config, util/command
app_sizes.rs ← util/classify, util/background
app_git.rs ← util/command, util/config
app_archive.rs ← util/command, util/cleanup
app_remote.rs ← util/command, util/cleanup, util/config
app_shell.rs ← util/config, util/command
app_transfer.rs ← util/config, util/background
app_mouse.rs ← (no crate dependencies; UI-local)
app_notes.rs ← util/background
app_sqlite.rs ← util/command
integration/app.rs, probe.rs ← util/command, util/config
integration/rows.rs ← ui/palette

main.rs ← all (orchestrator)
```

## Copilot Routing Rules

### Configuration, Limits, or Environment Variables
→ Add to `util/config.rs::AppConfig`; parse once at startup; access via `app.config`

### Error-Safe File Operations (Cleanup, Delete, Unlink)
→ Use `util/cleanup.rs::safe_cleanup_path()` or `safe_unlink_path()`
→ **Never** write `let _ = fs::remove_file/dir()` — always handle the error

### Path Metadata Classification (is_dir, is_symlink, type)
→ Use `util/classify.rs::classify_path()` — **never** duplicate symlink_metadata checks

### External Command Execution (git, archive, preview tools)
→ Use `util/command.rs::CommandBuilder` methods
→ **Never** write raw `Command::new("git"/"fusermount"/"bat"...)` — use the builder

### Background Task Polling (copy, git, preview, folder size)
→ Use `util/background.rs::BackgroundTask<T>` for new tasks
→ **Never** create new separate `pump_*()` methods — consolidate in main loop

### Color/Styling Constants
→ Use `ui/palette.rs::Palette` constants and helper methods
→ **Never** hard-code `Color::Rgb(...)` — add to palette if missing

### Span/Text Construction
→ Use `ui/spans.rs` builders (`titled_span`, `status_span`, `entry_type_span`, etc.)
→ Prefer builders over raw `Span::styled()` calls for common patterns

### Modal Dialog or Popup (Help, Confirm, Bookmarks)
→ Implement `ui/modal.rs::Modal` trait or extend existing concrete modal

### Input Editing (Buffer, Cursor, Selection)
→ Extend `src/app_input.rs`

### Permission/Owner/Group/UID/GID Logic
→ Extend `src/app_meta.rs`

### Entry Row Rendering/Cache Fields
→ Extend `src/app_render_cache.rs` (uses classify, palette, spans)

### Search/Candidate Scanning
→ Extend `src/app_search.rs` (uses config for limits)

### File/Archive Type Classification
→ Extend `src/app_files.rs` (uses config for thresholds)

### Preview Pane Content + Async Preview Jobs
→ Extend `src/app_preview.rs` (preview cache, request/pump, text/image preview building)

### Folder Size Computation
→ Extend `src/app_sizes.rs` (uses classify, background)

### Git Status/Branch/Dirty Detection
→ Extend `src/app_git.rs` (uses command.rs)

### Git Commit/Push/Tag Flows
→ Extend `src/app_git.rs` (interactive diff preview, commit, push, and tag actions)

### Archive Mount/Preview/Unmount
→ Extend `src/app_archive.rs` (uses command.rs, cleanup.rs)

### Archive Create/Extract/Progress Flows
→ Extend `src/app_archive.rs` (zip create flow, extract flow, archive job progress pump)

### SSH/Rclone/Remote Mount Flows
→ Extend `src/app_remote.rs` (remote discovery, mount/unmount, remote path/header identity)

### Shell Runner + Split Tmux Flows
→ Extend `src/app_shell.rs` (split less/editor panes and interactive shell command runner)

### Clipboard Transfer / Copy-Move Progress
→ Extend `src/app_transfer.rs` (paste queue, copy worker lifecycle, transfer status updates, clipboard backend helpers)

### Mouse Event Handling (Click, Scroll, Drag)
→ Extend `src/app_mouse.rs` (mouse click dispatch, tab hit-test, scrollbar click/drag, confirm dialog click handlers)

### Integration Tool Detection
→ Extend `src/integration/probe.rs` (uses command.rs)

### Integration Installation/Flow
→ Extend `src/integration/app.rs` (uses config, command)

### AppMode State Transitions
→ Extend `src/app_modes.rs` — **never** scatter `self.mode = AppMode::*` in main.rs

### Startup/Initialization Logic
→ Extend `src/app_init.rs` — **never** add startup logic to main.rs

### Entry Filtering/Iteration
→ Use/extend `src/app_entry_iter.rs` — avoid raw index loops

### Image Caching
→ Extend `src/app_images.rs`

### Directory Notes (`.sb`) Loading/Edit/Save
→ Extend `src/app_notes.rs`

### SQLite Preview (tables + row rendering)
→ Extend `src/app_sqlite.rs`

### Directory Icon Mapping
→ Extend `src/ui/icons.rs`

### Panel/Tab Bar Rendering
→ Extend `src/ui/panels.rs`

### Status Footer Icons/Decoration
→ Extend `src/ui/status.rs`

### Non-TUI CLI Output/List Mode
→ Extend `src/ui/cli.rs`

**Only keep code in `src/main.rs` if it is:**
- Crossterm raw mode setup/teardown
- Ratatui event loop and render dispatch
- App struct creation (via `app_init`)
- Exit cleanup (restoring terminal, writing `/tmp/sb_path`)

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` 0.26 | TUI layout and widgets |
| `crossterm` 0.27 | Raw mode, alternate screen, key events |
| `clap` 4 | CLI argument parsing (derive macros) |
| `devicons` | File-type Unicode icons (Dark theme) |
| `chrono` | Modification timestamp formatting |
| `hostname` | Prompt display |
| `std::sync::mpsc` | Background task channels |

## Conventions

- **Platform guards**: use `#[cfg(unix)]` / `#[cfg(not(unix))]` for OS-specific code
- **Colors**: use `ui/palette.rs::Palette` constants — **never** raw ANSI escape literals or hard-coded `Color::Rgb(...)`
- **Error handling**: use `util/cleanup.rs` for fs cleanup; avoid silent `let _ = fs::*`
- **Config**: read from `util/config.rs::AppConfig` via `app.config` — parse once at startup
- **No async**: all I/O is blocking — keep that way unless refactoring event loop
- **Visibility discipline**: default to `pub(crate)` for extracted methods; re-export via mod.rs

## Pitfalls

- **Raw terminal mode** must always be restored; wrap any new panic paths with `crossterm::terminal::disable_raw_mode()` + `LeaveAlternateScreen`
- **Git subprocess** runs on every status refresh — avoid calling it in tight loops
- **Hardcoded column widths** (40% / 12 / 8 / ≥20) can overflow on narrow terminals
- **Clipboard holds absolute `PathBuf`s** — paste always targets current directory
- **Config is parsed once** — do not call `env::var()` directly outside `util/config.rs`
- **Background tasks need polling** — main loop calls `pump_background_tasks()` each cycle
- **Mode transitions** go through `app_modes.rs` — never assign `self.mode` directly in scattered code

## Quick Lookup Table

| Task | Module |
|------|--------|
| Add env var or config limit | `util/config.rs` |
| Delete file safely (w/ error) | `util/cleanup.rs` |
| Check path is dir/symlink | `util/classify.rs` |
| Run git/archive/preview command | `util/command.rs` |
| Poll background task channel | `util/background.rs` |
| Format file size or ETA | `util/format.rs` |
| Add/change a theme color | `ui/palette.rs` |
| Build a styled span/text | `ui/spans.rs` |
| Add/change a dialog box | `ui/modal.rs` |
| Panel/tab rendering | `ui/panels.rs` |
| Search highlighting | `ui/search.rs` |
| Status footer icons | `ui/status.rs` |
| List-mode CLI output | `ui/cli.rs` |
| App startup initialization | `app_init.rs` |
| Mode state transitions | `app_modes.rs` |
| Entry filtering/iteration | `app_entry_iter.rs` |
| Text input & cursor editing | `app_input.rs` |
| Permissions/ownership | `app_meta.rs` |
| Entry row rendering/cache | `app_render_cache.rs` |
| Search/candidate scan | `app_search.rs` |
| File type detection | `app_files.rs` |
| Preview pane logic | `app_preview.rs` |
| Folder walk / size calc | `app_sizes.rs` |
| Git branch/dirty status | `app_git.rs` |
| Git commit/push/tag workflows | `app_git.rs` |
| Archive mount/unmount | `app_archive.rs` |
| Archive create/extract/progress | `app_archive.rs` |
| Remote mount workflows | `app_remote.rs` |
| Shell split/runner flows | `app_shell.rs` |
| Clipboard transfer pipeline | `app_transfer.rs` |
| Mouse event handling (click, scroll, drag) | `app_mouse.rs` |
| Image cache | `app_images.rs` |
| Directory notes | `app_notes.rs` |
| SQLite preview | `app_sqlite.rs` |
| Tool probing | `integration/probe.rs` |
| Integration install flow | `integration/app.rs` |
| Integration UI rows | `integration/rows.rs` |
| Integration definitions | `integration/catalog.rs` |
