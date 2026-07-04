<p align="center">
  <img src="docs/images/logo.png" alt="shell buddy - terminal file manager" width="491">
</p>

<p align="center">
  <a href="https://crates.io/crates/shell-buddy"><img src="https://img.shields.io/crates/v/shell-buddy.svg?style=flat-square&color=orange" alt="Crates.io Version" /></a>
  <a href="https://crates.io/crates/shell-buddy"><img src="https://img.shields.io/crates/d/shell-buddy.svg?style=flat-square&color=blue&label=crates%20downloads" alt="Crates.io Downloads" /></a>
  <a href="https://github.com/hjelev/sb/releases"><img src="https://img.shields.io/github/downloads/hjelev/sb/total.svg?style=flat-square&color=brightgreen&label=github%20downloads&kill_cache=1" alt="GitHub Downloads" /></a>
  <a href="https://github.com/hjelev/sb/actions/workflows/release.yml"><img src="https://github.com/hjelev/sb/actions/workflows/release.yml/badge.svg?branch=master&style=flat-square" alt="Build Status" /></a>
  <a href="https://github.com/hjelev/sb/blob/master/LICENSE"><img src="https://img.shields.io/github/license/hjelev/sb.svg?style=flat-square&color=purple" alt="License" /></a>
  <a href="https://github.com/hjelev/sb/stargazers"><img src="https://img.shields.io/github/stars/hjelev/sb.svg?style=flat-square&color=gold" alt="GitHub Stars" /></a>
</p>


![Shell Buddy screenshot](docs/images/screenshot-1.png)

<details>
  <summary>Show screenshots</summary>
  <img src="docs/images/screenshot-2.png" alt="lyrtui screenshot 3" width="926"/>
  <img src="docs/images/screenshot-3.png" alt="lyrtui screenshot 4" width="926"/>
  <img src="docs/images/screenshot-4.png" alt="lyrtui screenshot 5" width="926"/>
  <img src="docs/images/screenshot-5.png" alt="lyrtui screenshot 6" width="926"/>
  <img src="docs/images/screenshot-6.png" alt="lyrtui screenshot 7" width="926"/>
  <img src="docs/images/screenshot-7.png" alt="lyrtui screenshot 8" width="926"/>
  <img src="docs/images/screenshot-8.png" alt="lyrtui screenshot 8" width="926"/>
  <img src="docs/images/screenshot-9.png" alt="lyrtui screenshot 8" width="926"/>
  <img src="docs/images/screenshot-10.png" alt="lyrtui screenshot 8" width="926"/>
  <img src="docs/images/screenshot-11.png" alt="lyrtui screenshot 8" width="926"/>
</details>

A terminal file manager (TUI) written in Rust using `ratatui` + `crossterm`.

`sb` (Shell Buddy) is a keyboard-driven explorer focused on fast local navigation with optional integrations for previews, archive handling, searching, remote mounts, and lightweight Git workflows.

## What sets sb apart

- **In-TUI Git workflow** — diff preview → status review → commit → push → optional tag, all without leaving the file manager (`G`)
- **AI commit messages** — draft a commit message from the diff with one keypress (Groq or GitHub Models), then edit before committing (`Ctrl+G` in the commit prompt)
- **AI folder organization** — propose grouping the current folder's files into new or existing subfolders, review the plan, then confirm before anything moves (`Ctrl+O`)
- **Custom shortcuts** — rebind any of the 42 browsing commands from the built-in Shortcuts panel; changes persist in the config and the help screen and footer pills update to match
- **Inline path filters** — type `^prefix`, `suffix$`, or `~contains` directly in the path bar to live-filter the listing (`Tab`)
- **Integration manager with one-key install** — see which optional tools are missing and install them via Homebrew without leaving the TUI (`I`)
- **Age encryption** — protect or decrypt `.age` files in-place with a single keypress (`p`)
- **Per-file notes** — attach notes to any file, stored in a hidden `.sb` file per directory (`Ctrl+n`)
- **tmux-aware splits** — `i` opens a shell + preview pane; `E` opens a shell + editor pane (falls back to `zellij` when tmux isn't installed)
- **Clipboard edit** — `Ctrl+e` opens the current clipboard contents in `$EDITOR` for quick manipulation
- **CLI list/tree modes** — `sb -l`, `sb -t`, `sb -l2` produce TUI-consistent column output; `sb <file>` skips the TUI and opens with the best available viewer

---

<details>
<summary><strong>Build and Run</strong></summary>

```bash
cargo build
cargo run
```

Release build:

```bash
cargo build --release
```

Release binary path:

```text
target/release/sb
```

List mode examples:

```bash
# Current directory
sb -l

# Include hidden entries
sb -a
sb -la

# Recursive display size + percent share columns
sb -l --total-size

# Full tree output
sb -t

# Tree output limited to depth 2
sb -l2
sb -l 2

# Path can appear before or after --total-size
sb -la /var/log --total-size
sb --total-size -l /var/log

# Open a file directly with the best available previewer/viewer
sb README.md
sb diagram.mmd

# Open a file with pager mode enabled
sb -l README.md

# Open a file in $EDITOR (fallback: nano)
sb -e README.md
```

</details>

<details>
<summary><strong>CLI List Mode</strong></summary>

- `-l [PATH]`: list directory entries and exit.
- `-a [PATH]`: same as `-l`, but includes hidden files.
- `-la [PATH]`: same as `-l`, but includes hidden files.
- `-e [FILE]`: open file in `$EDITOR` (fallback: `nano`) and exit.
- `-t [PATH]`: tree-list recursively (full depth) and exit.
- `-lN [PATH]` / `-l N [PATH]`: tree-list to depth `N` and exit.
- `--total-size`: when used with `-l`, `-a`, or `-la`, shows recursive display size for each entry and a `%` column with that entry's share of the listed total.

Notes:

- `PATH` is optional and can be placed after `-l`/`-a`/`-la` or after `--total-size`.
- The list output reuses the file manager's auto-calculated owner/group column widths for consistent alignment.
- When invoked as `sb <FILE>`, the app skips the TUI and opens the file directly with best-available viewer output (no pager).
- When invoked as `sb -l <FILE>`, direct file mode uses pager-enabled output.

</details>

<details>
<summary><strong>Installation</strong></summary>

### From crates.io

```bash
cargo install shell-buddy
```

### From Homebrew

```bash
brew install hjelev/tap/sb
```

### From Source

```bash
git clone https://github.com/hjelev/sb.git
cd sb
cargo install --path .
```

### From Releases

Prebuilt binaries and the auto-installer script are available in [GitHub Releases](https://github.com/hjelev/sb/releases).
Use the installer there if you want the fastest setup without building from source.

</details>

<details>
<summary><strong>Core Controls</strong></summary>

The keys below are the defaults — most browsing commands can be rebound from the **Shortcuts** panel (see *Custom Shortcuts* below). Structural keys (arrows, `Enter`, `Esc`, `Tab`, `Space`, `PageUp`/`PageDown`, `Home`/`End`, digit bookmarks, and the fixed F-key/`Del` alternates) are not rebindable.

- `q` / `Esc`: quit
- `\``: toggle modes
- `Enter` / `Right`: open entry / preview file
- `Left` / `Backspace`: go to parent / leave mounted view
- mouse left-click: select clicked entry
- mouse double left-click: open clicked entry (same behavior as `Right`)
- mouse right-click: go to parent / leave mounted view (same behavior as `Left`)
- `Up`/`Down`/`PageUp`/`PageDown`/`Home`/`End`: navigation
- `Space`: mark/unmark current entry
- `*`: toggle all marks
- `c` or `F5`: copy to internal clipboard
- `Ctrl+c`: copy selected full path(s) to system clipboard
- `Ctrl+e`: edit system clipboard text in `$EDITOR`
- `v`: paste
- `m`: move (cut+paste behavior) from internal clipboard
- `d`: delete (with confirmation)
- `x`: toggle executable bit on selected file(s)
- `p`: protect/unprotect file with `age` (`.age`)
- `F2` or `r`: rename (or bulk rename with `vidir` when multiple are marked)
- `e` or `F4`: open in `$EDITOR` (or `hexedit` for binary if available)
- `E`: split tmux (or zellij) session with shell on the left and `$EDITOR` on the right (`Ctrl+e` is clipboard edit)
- `n`: new file or folder (folder starts with `/`)
- `Ctrl+n`: add/edit note for selected item(s)
- `t`: open `~/.todo` in `$EDITOR` (creates it if missing)
- `Z`: archive create/extract flow
- `C`: compare marked file vs cursor file with `delta`
- `G`: Git commit workflow with diff preview, `git status`, commit/push, and optional post-push tag creation
- `o`: open with system GUI opener (`open` on macOS, `xdg-open`/`gio open` elsewhere)
- `f`: open Search overlay (filename search; uses built-in search if `fzf` is missing)
- `g`: content search (`rg`, optional `fzf` handoff; falls back to built-in Search content mode when `rg` is missing)
- `;`: open command prompt, run shell command, then wait for keypress before returning to TUI
- `S`: SSH/rclone remote picker
- `i`: split shell (left) + `less` preview (right 30%)
- `I`: integrations panel
- `b`: bookmarks panel
- `T`: themes panel
- `w`: download a URL into the current folder
- `Ctrl+z`: drop to interactive shell in current directory
- `/` (in browsing): quick-filter the current folder listing (not available in dual panel mode)
- `Tab` (in browsing): edit current path inline; supports `/path/^prefix`, `/path/suffix$`, and `/path/~contains` filters
- `Tab` / `Shift+Tab` in Help/Search/Bookmarks/Remote Mounts/Sorting/Integrations: cycle tabs forward/backward
- `s`: toggle folder size calculation in listing
- `Ctrl+s`: open sort mode menu
- `+`: expand selected/marked non-empty folder(s) by one tree level
- `-`: contract selected/marked folder(s) by one tree level
- quick `++`: expand selected/marked non-empty folder(s) to max depth
- quick `--`: collapse all opened folders in tree view
- `0-9`: jump to bookmark (`SB_BOOKMARK_0..9`)
- `.`: toggle hidden files
- `~`: jump to home
- `h`: help overlay

</details>

<details>
<summary><strong>Custom Shortcuts</strong></summary>

Open the help overlay (`h`) and cycle with `Tab`/`Shift+Tab` to the **Shortcuts** tab (it is the last tab, so `h` followed by `Shift+Tab` gets there directly).

The panel lists every rebindable browsing command grouped by category, showing the current key and the default:

- `Up`/`Down`/`PageUp`/`PageDown`/`Home`/`End`: move the selection
- `Enter` (or `Space`): capture a new key — the next key you press becomes the binding; `Esc` cancels the capture
- `Backspace` / `Del`: reset the selected command to its default key
- `Esc` / `q`: close the panel

Rules:

- Reserved structural keys (arrows, `Enter`, `Esc`, `Tab`, `Space`, `PageUp`/`PageDown`, `Home`/`End`, digits, and the fixed `F2`/`F4`/`F5`/`Del` alternates) cannot be assigned.
- A key already used by another command is rejected with a warning naming the conflicting command — rebind that one first if you want to swap keys.
- Changes apply immediately and persist as `shortcut_<id> = <combo>` lines in `~/.config/sb/config` (e.g. `shortcut_rename = u`, `shortcut_sort_menu = ctrl+t`). You can edit these lines by hand; invalid, reserved, or conflicting entries fall back to defaults on load.
- The help screen and the footer shortcut pills always reflect the active bindings.

</details>

<details>
<summary><strong>Search Overlay Functions</strong></summary>

When Search is open (`f` or fallback from `g`):

- `Up` / `Down`: move result selection
- `Enter`: open selected match
- `Esc`: close Search
- `Ctrl+t`: toggle scope between `Filename` and `Content`
- Query supports regex forms: `re:pattern` or `/pattern/i`
- Content-mode results render as `path:line` with highlighted matching snippets
- Content-mode scanning runs asynchronously (UI remains responsive)

Content limits editor (content scope):

- `Ctrl+l`: open/close limits editor
- `Up` / `Down`: select which limit to edit
- `Left` / `Right` or `-` / `+`: decrease/increase selected limit
- `Shift` + adjust: 10x step
- `r`: reset limits from environment/default values
- `Enter` / `Esc`: close limits editor

</details>

<details>
<summary><strong>Path Editing and Filters</strong></summary>

Press `Tab` while browsing to edit the current path in place.

- Enter a directory path and press `Enter` to jump there.
- Add a suffix filter to keep the current directory but narrow visible entries:
    - `/some/path/^foo`: names starting with `foo`
    - `/some/path/bar$`: names ending with `bar`
    - `/some/path/~baz`: names containing `baz`
- `Esc` from path-edit mode clears the active filter and returns to browsing.

The active filter remains visible in the header until you change directories.

</details>

<details>
<summary><strong>Git Workflow</strong></summary>

Press `G` (or `Ctrl+G`) in a Git working tree to:

- preview the current diff (`delta` side-by-side when available)
- view `git status`
- confirm whether to continue
- enter a commit message inside the TUI — or, once in the commit prompt, press `Ctrl+G` again to generate one with AI from the current diff (the draft stays editable; `Ctrl+G` again retries)
- auto-run `git add --all`, `git commit`, and `git push origin HEAD`
- optionally press `t` immediately after a successful push to create and push a tag

When tagging, the tag input box is prefilled from the latest reachable Git tag when one exists.

AI commit messages use an OpenAI-compatible chat-completions API (Groq or GitHub Models). Pick the provider, model, and API key in the **Settings** panel (open the help overlay with `h`, then `Tab`/`Shift+Tab` to reach Settings). If no key is stored there, the provider's environment variable is used as a fallback (`GROQ_API_KEY` or `GITHUB_TOKEN`).

**Getting an API token:**

- **Groq**: sign up at [console.groq.com](https://console.groq.com), then create a key on the [API Keys page](https://console.groq.com/keys). Groq has a free tier. Paste the key into the Settings panel, or export it as `GROQ_API_KEY`.
- **GitHub Models**: create a [fine-grained personal access token](https://github.com/settings/personal-access-tokens/new) (or a classic token) — no extra scopes are required, just a valid token from a GitHub account with access to [GitHub Models](https://github.com/marketplace/models). Paste it into the Settings panel, or export it as `GITHUB_TOKEN`.

</details>

<details>
<summary><strong>AI Organize</strong></summary>

Press `Ctrl+O` on the active pane's current folder to:

- send the folder's top-level entry names to the same AI provider configured for commit messages (see **Settings** above)
- review the proposed plan — new or reused subfolders and which entries would move into each
- press **Confirm** to create the folders and move the entries, or **Cancel**/`Esc` to discard the plan with no filesystem changes

Nothing is applied until you explicitly confirm. Hidden (dot) entries are never included, and any AI-proposed name that doesn't match a real entry, or any folder name containing a path separator, is dropped before the plan is even shown.

</details>

<details>
<summary><strong>Integrations</strong></summary>

Required behavior:

- `less`: file viewing fallback
- `$EDITOR`: file editing command (defaults to `nano` if unset)

Optional integrations (auto-detected, toggle in `I` panel):

- In the Integrations panel, pressing `Enter` on a missing integration asks for confirmation and can install with Homebrew when available (macOS and Linux/Homebrew).
- Press `/` inside the Integrations panel to search/filter the integration list.

- VCS: `git`
- Viewers/previews: `bat`, `glow`, `mmdflux`, `jnv`, `csvlens`, `hexyl`, `chafa`, `viu`, `sox`, `pdftotext`, `asciinema`, `links`
- Diff/edit helpers: `delta`, `hexedit`, `vidir`, `tmux`, `zellij`
- Archives: `zip`/`unzip`, `tar`, `7z` family (`7z`/`7zz`/`7zr`), `rar`/`unrar`, `fuse-zip`, `archivemount`
- Security: `age`
- Remote mounts: `sshfs`, `rclone`
- Search: `rg`, `fzf`
- Clipboard backends: `wl-copy`/`wl-paste`, `xclip`, `xsel`, `pbcopy`/`pbpaste`

Remote picker (`S`) also lists existing local mounted folders discovered under:

- `/media/$USER`
- `/run/media/$USER`
- `/mnt`
- `/run/user/$UID/gvfs`

If an optional tool is not available, the feature is skipped or falls back gracefully.

</details>

<details>
<summary><strong>Environment Notes</strong></summary>

- `NERD_FONT_ACTIVE=1`: enable Nerd Font icons
- `NO_COLOR=1`: disable file name colors (modifiers like bold/dim still apply)
- `TERMINAL_ICONS=0`: hide all file icons (Nerd Font glyphs and emoji)
- `EDITOR`: editor command used by `e`/`F4`, `E`, `Ctrl+e`, and `t`
- `SB_BOOKMARK_0` ... `SB_BOOKMARK_9`: bookmark directories
- `SB_SEARCH_CONTENT_MAX_FILES`: built-in Search content-mode max files scanned (default: `20000`)
- `SB_SEARCH_CONTENT_MAX_HITS`: built-in Search content-mode max matches returned (default: `2000`)
- `SB_SEARCH_CONTENT_MAX_FILE_BYTES`: built-in Search content-mode per-file byte cap (default: `2097152` / 2 MiB)
- `GROQ_API_KEY`: API key fallback for AI commit messages when the Groq provider is selected and no key is stored in config
- `GITHUB_TOKEN`: API key fallback for AI commit messages when the GitHub Models provider is selected and no key is stored in config

Persistent config (`~/.config/sb/config`, `key = value`) also supports:

- `disable_clock = true`: replace the header clock with a disk-usage pill
- `ai_provider = groq`: AI commit message provider (`groq` or `github`; default `groq`)
- `ai_model = ...`: model for AI commit messages (leave empty to use the provider default)
- `ai_api_key = ...`: API key for AI commit messages (overrides the provider environment variable; usually set via the Settings panel)
- `shortcut_<id> = <combo>`: custom key binding for a browsing command (e.g. `shortcut_rename = u`, `shortcut_sort_menu = ctrl+t`; usually set via the Shortcuts panel)
- Nerd Fonts and file name colors can be toggled live from the Themes menu (the choice is persisted here alongside the active theme and view mode)

</details>

<details>
<summary><strong>Shell Integration</strong></summary>

To enable automatic directory change on exit, add the following function to your shell configuration file (e.g., `~/.bashrc`, `~/.zshrc`):

```bash
sb() {
    "$HOME/.cargo/bin/sb" "$@"
    sb_last_path="${XDG_RUNTIME_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}}/sb/last_path"
    if [ -f "$sb_last_path" ]
    then
        cd "$(cat "$sb_last_path")"
        rm -i -f "$sb_last_path"
    fi
}
```

After adding the function, reload your shell configuration:

```bash
source ~/.bashrc  # or source ~/.zshrc
```

</details>

<details>
<summary><strong>Project Structure</strong></summary>

Current code layout is modular:

- `src/main.rs`: app state, event loop, orchestration, and top-level workflows
- `src/app_input.rs`: input editing helpers
- `src/app_meta.rs`: permissions/owner/group metadata helpers
- `src/app_render_cache.rs`: entry render-cache generation
- `src/app_search.rs`: built-in search and path-filter matching helpers
- `src/app_files.rs`: file-type classification helpers
- `src/app_sizes.rs`: folder-size and aggregate-size scanning helpers
- `src/app_git.rs`: Git status/background cache helpers
- `src/app_archive.rs`: archive mount and preview lifecycle helpers
- `src/integration/`: integration catalog, probing, rows, and install flow
- `src/ui/`: CLI output, icons, panels, search spans, and status rendering
- `src/util/`: shared formatting helpers
- `Cargo.toml`: dependencies and release profile settings

</details>

<details>
<summary><strong>Dependencies</strong></summary>

From `Cargo.toml`:

- `ratatui` (UI)
- `crossterm` (terminal events/raw mode)
- `chrono` (timestamps)
- `devicons` (file icons)
- `hostname` (header prompt)
- `users` (owner metadata)
- `clap` (present as dependency)
- `regex` (search regex mode)
- `rayon` (parallel entry render-cache build)
- `unicode-width` (display-width-aware list-mode alignment)

</details>
