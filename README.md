# Shell Buddy (`sb`)

A lightweight terminal file browser written in Bash.

`sb` gives you a fast, keyboard-driven way to move through directories, open files, and copy/paste items without leaving the terminal.

## Features

- Arrow-key navigation (`↑/↓`) through files and folders
- Open directory or file with `→` or `Enter`
- Go back with `←`
- Jump to home directory with `~`
- Copy (`c`) and paste (`p`) files/directories
- Preserves cursor position per directory while navigating
- Optional image preview in terminal (via `chafa`)
- Displays owner, permissions, size, and modified time
- Optional export of final directory path on exit

## Requirements

- Linux or Unix-like environment
- `bash`
- Core utils used by the script (`ls`, `cp`, `sed`, `awk`, `tput`, `stty`)
- Optional: `xdg-open` (for opening files in a graphical session)
- One of `nano`, `vim`, `vi`, `less`, or another editor via `$EDITOR`/`$VISUAL` for headless servers
- Optional: `chafa` (for inline image preview)

## Installation

### Option 1: One-command install

Install the latest release:

```bash
curl -fsSL https://raw.githubusercontent.com/hjelev/sb/master/install.sh | bash
```
or
```bash
curl -fsSL https://bit.ly/sb-install | bash
```

The installer detects Bash/Zsh and automatically adds an `sb()` shell function
to `~/.bashrc` or `~/.zshrc` so `sb` can return you to the last visited folder.

To skip shell integration:

```bash
curl -fsSL https://raw.githubusercontent.com/hjelev/sb/master/install.sh | bash -s -- --no-shell-setup
```

Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/hjelev/sb/master/install.sh | bash -s -- --version v0.1.0
```

By default this installs `sb` into the first writable directory already on your `PATH`.
If no writable `PATH` entry exists, it falls back to a user-local bin directory
(`$XDG_BIN_HOME`, `~/bin`, `~/.local/bin`, then `/usr/local/bin`) and, when shell setup is enabled,
adds a matching `PATH` export to your shell config automatically.
To use a different location:

```bash
curl -fsSL https://raw.githubusercontent.com/hjelev/sb/master/install.sh | SB_INSTALL_DIR=/usr/local/bin bash
```

The installer tries the latest GitHub release first and falls back to `master` or `main` until the first release exists.
When installing from a release tag (automatic latest release or `--version`), the installer also stamps that tag version into the installed `sb` script so `sb --version` matches the installed release.

### Uninstall

Remove `sb` and shell integration:

```bash
curl -fsSL https://raw.githubusercontent.com/hjelev/sb/master/install.sh | bash -s -- --uninstall
```

If you installed to a custom directory, pass the same location during uninstall:

```bash
curl -fsSL https://raw.githubusercontent.com/hjelev/sb/master/install.sh | SB_INSTALL_DIR=/usr/local/bin bash -s -- --uninstall
# or
curl -fsSL https://raw.githubusercontent.com/hjelev/sb/master/install.sh | bash -s -- --uninstall --install-dir /usr/local/bin
```

### Option 2: Run directly

```bash
chmod +x sb
./sb
```

### Option 3: Install in your PATH manually

```bash
chmod +x sb
sudo cp sb /usr/local/bin/sb
sb
```

### Option 4: Add `sb` as a shell function manually

If you skipped shell setup or use an unsupported shell, add this function manually.

For Bash (`~/.bashrc`) or Zsh (`~/.zshrc`):

```bash
sb() {
	if [ "$#" -gt 0 ]; then
		bash "/usr/local/bin/sb" "$@"
		return
	fi

	local tmp_file
	tmp_file=$(mktemp)
	bash "/usr/local/bin/sb" --export-path "$tmp_file"
	if [ -s "$tmp_file" ]; then
		cd "$(cat "$tmp_file")"
	fi
	rm -f "$tmp_file"
}
```

Then reload your shell:

```bash
source ~/.bashrc
# or
source ~/.zshrc
```

## Usage

Start in the current directory:

```bash
sb
```

Print the installed version:

```bash
sb --version
```

Update/reinstall `sb` in place using the installer:

```bash
sb --update
```

Start and export your last visited directory when quitting:

```bash
sb --export-path /tmp/last_dir.txt
```

After you quit (`q`), the script writes the current working directory to the export file.

## Keybindings

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection |
| `→` or `Enter` | Open directory / file |
| `←` | Go to parent directory |
| `~` | Jump to `$HOME` |
| `c` | Copy selected file/folder into clipboard |
| `p` | Paste clipboard into current directory |
| `m` | Move clipboard item into current directory |
| `n` | Create a new file |
| `N` | Create a new folder |
| `l` | Open selected file in `less` |
| `x` | Toggle executable permission on selected item |
| `d` | Delete selected item |
| `.` | Toggle hidden files |
| `h` | Show help screen |
| `q` | Quit |

## Notes

- If a paste target name already exists, `sb` prompts for a new name.
- For images (`jpg`, `png`, `gif`, etc.), `sb` uses `chafa` if available; otherwise it falls back to the normal file-open flow.
- On headless Linux systems, `sb` falls back to `$VISUAL`, `$EDITOR`, `sensible-editor`, `editor`, `nano`, `vim`, `vi`, `less`, or `more`.
- UI adapts to terminal resize events.

## Troubleshooting

- If files do not open in a GUI session, make sure `xdg-open` is available.
- On servers, set `$EDITOR` or install a terminal editor such as `nano` or `vim`.
- If image preview is not shown in terminal, install `chafa`.
- If colors/controls look wrong, try running in a standard terminal emulator with ANSI support.

## License

No license file is currently included in this repository.
