#!/usr/bin/env bash
set -euo pipefail

REPO="hjelev/sb"
DEFAULT_FALLBACK_REFS=("master" "main")
INSTALL_DIR="${SB_INSTALL_DIR:-$HOME/.local/bin}"
VERSION=""
REF=""
SETUP_SHELL="${SB_SETUP_SHELL:-1}"

usage() {
    cat <<'EOF'
Usage: install.sh [--version TAG] [--ref GIT_REF] [--install-dir DIR] [--no-shell-setup]

Installs sb into a directory on your PATH.

Options:
  --version TAG      Install a tagged version, for example v0.1.0.
  --ref GIT_REF      Install from a git ref such as master, main, or a commit SHA.
  --install-dir DIR  Install destination. Defaults to ~/.local/bin or SB_INSTALL_DIR.
  --no-shell-setup   Do not add the sb() shell function to your shell rc file.
  --help             Show this help text and exit.

If neither --version nor --ref is provided, the installer tries the latest GitHub
release first and falls back to the default branch when no release exists yet.
EOF
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Error: required command '$1' is not installed." >&2
        exit 1
    fi
}

latest_release_tag() {
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | awk -F '"' '/"tag_name"/ { print $4; exit }'
}

ref_has_script() {
    local ref="$1"
    local url="https://raw.githubusercontent.com/$REPO/$ref/sb"
    curl -fsSI "$url" >/dev/null 2>&1
}

resolve_default_ref() {
    local candidate
    for candidate in "${DEFAULT_FALLBACK_REFS[@]}"; do
        if ref_has_script "$candidate"; then
            echo "$candidate"
            return 0
        fi
    done
    return 1
}

ensure_path_hint() {
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *)
            printf 'Warning: %s is not currently on your PATH.\n' "$INSTALL_DIR" >&2
            printf 'Add this line to your shell config:\n  export PATH="%s:$PATH"\n' "$INSTALL_DIR" >&2
            ;;
    esac
}

detect_shell_rc_file() {
    local shell_name
    shell_name="$(basename "${SHELL:-}")"

    case "$shell_name" in
        bash)
            echo "$HOME/.bashrc"
            ;;
        zsh)
            echo "$HOME/.zshrc"
            ;;
        *)
            return 1
            ;;
    esac
}

append_shell_integration() {
    local rc_file="$1"
    local install_path="$2"
    local marker_start="# >>> sb shell integration >>>"

    if [[ -f "$rc_file" ]] && grep -Fq "$marker_start" "$rc_file"; then
        printf 'Shell integration already exists in %s\n' "$rc_file"
        return 0
    fi

    if [[ -e "$rc_file" && ! -w "$rc_file" ]]; then
        printf 'Warning: cannot write to %s, skipping shell integration.\n' "$rc_file" >&2
        return 0
    fi

    mkdir -p "$(dirname "$rc_file")"
    cat >> "$rc_file" <<EOF

# >>> sb shell integration >>>
sb() {
    local tmp_file
    tmp_file=\$(mktemp)
    "$install_path" --export-path "\$tmp_file"
    if [ -s "\$tmp_file" ]; then
        cd "\$(cat "\$tmp_file")"
    fi
    rm -f "\$tmp_file"
}
# <<< sb shell integration <<<
EOF

    printf 'Added shell integration to %s\n' "$rc_file"
    printf 'Reload with: source %s\n' "$rc_file"
}

setup_shell_integration() {
    local install_path="$1"
    local rc_file

    if [[ "$SETUP_SHELL" == "0" ]]; then
        printf 'Skipping shell integration (--no-shell-setup or SB_SETUP_SHELL=0).\n'
        return 0
    fi

    if ! rc_file="$(detect_shell_rc_file)"; then
        printf 'Skipping shell integration: unsupported shell (%s).\n' "${SHELL:-unknown}"
        return 0
    fi

    append_shell_integration "$rc_file" "$install_path"
}

while (($# > 0)); do
    case "$1" in
        --version)
            if [[ -z "${2-}" ]]; then
                echo "Error: --version requires a tag." >&2
                exit 1
            fi
            VERSION="$2"
            shift 2
            ;;
        --ref)
            if [[ -z "${2-}" ]]; then
                echo "Error: --ref requires a git ref." >&2
                exit 1
            fi
            REF="$2"
            shift 2
            ;;
        --install-dir)
            if [[ -z "${2-}" ]]; then
                echo "Error: --install-dir requires a directory." >&2
                exit 1
            fi
            INSTALL_DIR="$2"
            shift 2
            ;;
        --no-shell-setup)
            SETUP_SHELL="0"
            shift
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            echo "Error: unknown option '$1'." >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ -n "$VERSION" && -n "$REF" ]]; then
    echo "Error: use either --version or --ref, not both." >&2
    exit 1
fi

require_cmd curl
require_cmd chmod
require_cmd mkdir

if [[ -n "$VERSION" ]]; then
    REF="$VERSION"
elif [[ -z "$REF" ]]; then
    if VERSION="$(latest_release_tag 2>/dev/null)" && [[ -n "$VERSION" ]]; then
        REF="$VERSION"
        printf 'Installing sb %s\n' "$VERSION"
    elif REF="$(resolve_default_ref)"; then
        printf 'No GitHub release found yet, installing from %s\n' "$REF"
    else
        echo "Error: no release found and no fallback branch with sb script is accessible." >&2
        exit 1
    fi
fi

mkdir -p "$INSTALL_DIR"
TMP_FILE="$(mktemp)"
trap 'rm -f "$TMP_FILE"' EXIT

DOWNLOAD_URL="https://raw.githubusercontent.com/$REPO/$REF/sb"
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_FILE"
chmod 0755 "$TMP_FILE"
mv "$TMP_FILE" "$INSTALL_DIR/sb"

printf 'Installed sb to %s/sb\n' "$INSTALL_DIR"
ensure_path_hint
setup_shell_integration "$INSTALL_DIR/sb"
printf 'Run: sb --version\n'