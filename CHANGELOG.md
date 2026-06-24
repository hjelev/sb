# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Four new colorful bundled theme skins: Synthwave, Coral Reef, Cyberpunk Neon, and Bubblegum Pop

## [0.8] - 2026-06-25

### Added
- Seven new themes: AIberto, Dracula, Rosé Pine, Everforest, Kanagawa, One Dark, Bannana
- Option to disable the header clock and show a disk-usage pill instead (`disable_clock` in `~/.config/sb/config`)

### Changed
- Improve free disk space display in the header
- Improve mouse support
- Make CLI list/tree output follow the GUI configs
- Redesign shortcut pills (rounding suppressed when Nerd Fonts are disabled)
- Performance improvements
- Large internal refactors: render submodules, key-dispatch modules, `SizeState` grouping, centralized TUI colors via `ThemeSpec`

### Fixed
- Dual panel refresh bug

## [0.7] - 2026-06-18

### Added
- Press `/` to quick-filter the current folder listing
- Search (`/`) inside the Integrations panel
- `zellij` support as a tmux-split fallback
- Toggle Nerd Fonts and file name colors directly from the Themes menu
- Delete env-defined bookmarks with `d`
- Deploy all theme skins during installation
- Auto dark/light mode for the HTML docs site

### Changed
- Git workflow now pulls before pushing
- Improve release pipeline
- Code de-duplication and restructuring across modules

### Fixed
- Bug in git integration
- Integrations search no longer blocked toggling with `Space`
- Homebrew install notes in release output
- Assorted docs fixes

## [0.6] - 2026-05-17

### Added
- Dual panel mode with independent navigation
- SSH/rclone remote host picker
- Persistent config: saves view mode and theme across sessions
- Themes support: Nord, Solarized, Gruvbox with pill selector
- Direct SSH connection option to remote hosts

### Changed
- Improve git status detection
- Add line numbers for text file preview
- Adjust header spacing and footer shortcut design
- Make folder names change color with the active theme
- Improve pills design and dual panel tree view bugs

### Fixed
- Multiple dual panel mode bugs
- Fix `o` shortcut in dual panel mode

## [0.5] - 2026-04-26

### Added
- Directory tree view (expand/collapse with `+`/`-`)
- Mouse support (click navigation, scroll)
- Preview mode (`` ` `` key) with image preview via sixel/`chafa`
- File download functionality with progress bar
- Scrollbar in large folders
- OS icon in header
- `resvg` integration for SVG image preview
- Rounded corners to menus and redesigned delete dialog

### Changed
- Improve `bat` integration in preview and tab behaviour
- Improve image preview rendering
- Improve header and footer look and feel
- Improve CLI parameters handling and size sorting
- Rework integrations screen scrolling
- Code restructure across multiple modules

### Fixed
- Bug in mouse support
- Shortcut bug fixes

## [0.4] - 2026-04-21

### Added
- SQLite database preview via `sqlite3`
- Git tag creation after `G` commit workflow
- `mmdflux` integration for `.mmd` file preview
- Filter support for path in header via Tab
- Press `t` to edit `~/.todo` text file
- Press `B` to edit clipboard contents
- Press `H` to view git history
- Press `E` to open file in editor with split terminal
- Open a file from CLI — launches associated viewer directly
- `--total-size` option in list mode
- Homebrew installer integration for macOS
- Display current time and date in header

### Changed
- Improve git integration in header (branch, status, commit workflow)
- Improve UI coloring and disk space display
- Update Nerd Font icons for folders and status messages
- Improve help screen and README with screenshots
- Restructure project layout

## [0.3] - 2026-04-21

### Added
- Investigation mode (`I` key) for detailed file metadata
- Group info displayed in metadata view

### Fixed
- Bug fix and improved column spacing

## [0.2] - 2026-04-15

### Added
- Internal file search (similar to `fzf`)
- Note system that saves notes per folder in a `.sb` file
- HTML preview with `links`
- Plain preview with `l`
- Drop to Shell feature
- Execute arbitrary bash command from within the app
- `.cast` file preview via `asciinema`
- `pdftotext` integration for PDF preview
- `NO_COLOR` and `TERMINAL_ICONS` environment variable support
- `ctrl+s` shortcut for sorting menu
- Press `s` for recursive folder size calculation
- Raspberry Pi armv7 support
- Improved `-l` / `-la` list mode with path argument support

### Changed
- Non-blocking recursive folder size scanning
- Improve macOS compatibility
- Speed up disk size calculation
- Improve folder icons and scrolling behaviour
- Improve git integration
- Improve integrations menu (scrolling, design)
- Make Help, Bookmarks, SSH mounts, and Integrations menus tab-style
- Add `%` to folder size display
- Beautify help screen

### Fixed
- Bug in `-l` display
- Bug in multi-file copy/move
- `fzf` integration fix for Raspberry Pi
- Fix `no_color` mode and mount exit behaviour
- Fix permissions alignment when user differs

## [0.1] - 2026-04-15

### Added
- Initial release: Shell Buddy rewritten in Rust
- Terminal UI built with `ratatui` + `crossterm`
- Keyboard-driven file manager with no runtime config files
- CI setup with `cargo-dist`

[Unreleased]: https://github.com/hjelev/sb/compare/v0.8.3...HEAD
[0.8]: https://github.com/hjelev/sb/compare/v0.7.6...v0.8.3
[0.7]: https://github.com/hjelev/sb/compare/v0.6.14...v0.7.6
[0.6]: https://github.com/hjelev/sb/compare/v0.5.19...v0.6.14
[0.5]: https://github.com/hjelev/sb/compare/v0.4.19...v0.5.19
[0.4]: https://github.com/hjelev/sb/compare/v0.3.2...v0.4.19
[0.3]: https://github.com/hjelev/sb/compare/v0.2.39...v0.3.2
[0.2]: https://github.com/hjelev/sb/compare/v0.1.0...v0.2.39
[0.1]: https://github.com/hjelev/sb/releases/tag/v0.1.0
