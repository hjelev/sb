#!/usr/bin/env bash
set -euo pipefail

REPO="hjelev/sb"
DEFAULT_BRANCH="main"
INSTALL_DIR="${SB_INSTALL_DIR:-$HOME/.local/bin}"
VERSION=""
REF=""

usage() {
    cat <<'EOF'
Usage: install.sh [--version TAG] [--ref GIT_REF] [--install-dir DIR]

Installs sb into a directory on your PATH.

Options:
  --version TAG      Install a tagged version, for example v0.1.0.
  --ref GIT_REF      Install from a git ref such as main or a commit SHA.
  --install-dir DIR  Install destination. Defaults to ~/.local/bin or SB_INSTALL_DIR.
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

ensure_path_hint() {
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *)
            printf 'Warning: %s is not currently on your PATH.\n' "$INSTALL_DIR" >&2
            printf 'Add this line to your shell config:\n  export PATH="%s:$PATH"\n' "$INSTALL_DIR" >&2
            ;;
    esac
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
    else
        REF="$DEFAULT_BRANCH"
        printf 'No GitHub release found yet, installing from %s\n' "$DEFAULT_BRANCH"
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
printf 'Run: sb --version\n'