# Simple Browser (`sb`)

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
- `xdg-open` (for opening non-image files externally)
- Optional: `chafa` (for inline image preview)

## Installation

### Option 1: Run directly

```bash
chmod +x sb
./sb
```

### Option 2: Install in your PATH

```bash
chmod +x sb
sudo cp sb /usr/local/bin/sb
sb
```

### Option 3: Add `sb` as a shell function (recommended)

If you want your terminal to `cd` into the last folder you visited after quitting `sb`, add this function to your shell config.

For Bash (`~/.bashrc`) or Zsh (`~/.zshrc`):

```bash
sb() {
	local tmp_file
	tmp_file=$(mktemp)
	# Change '/path/to/sb.sh' to the actual location (e.g., ~/sb.sh)
	bash "$HOME/sb" --export-path "$tmp_file"
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
| `h` | Show help line |
| `q` | Quit |

## Notes

- If a paste target name already exists, `sb` prompts for a new name.
- For images (`jpg`, `png`, `gif`, etc.), `sb` uses `chafa` if available; otherwise it falls back to `xdg-open`.
- UI adapts to terminal resize events.

## Troubleshooting

- If files do not open, make sure `xdg-open` is available.
- If image preview is not shown in terminal, install `chafa`.
- If colors/controls look wrong, try running in a standard terminal emulator with ANSI support.

## License

No license file is currently included in this repository.
