#!/bin/bash

# Format: "BinaryName:PackageManager:PackageName:Description"
# Note: 'logo-ls' is omitted as it requires a custom .deb download, not apt/cargo.
tools=(
    "eza:cargo:eza:Modern ls replacement"
    "lsd:cargo:lsd:LSDeluxe: colors and icons"
    "dust:cargo:du-dust:Disk usage tree"
    "git:apt:git:Version control"
    "sshfs:apt:sshfs:SSH filesystem"
    "fuse-zip:apt:fuse-zip:Open zip as folders"
    "zip:apt:zip:Zip archiver"
    "tar:apt:tar:Tar archiver"
    "7z:apt:p7zip-full:7-Zip archiver"
    "rar:apt:rar:RAR archiver"
    "delta:cargo:git-delta:Side-by-side diff"
    "jnv:cargo:jnv:JSON previewer"
    "pdftotext:apt:poppler-utils:PDF text preview"
    "hexyl:cargo:hexyl:Hex viewer"
    "hexedit:apt:hexedit:Hex editor"
    "rg:apt:ripgrep:Recursive search"
    "fzf:apt:fzf:Fuzzy finder"
    "wget:apt:wget:Network downloader"
    "curl:apt:curl:Network downloader"
    "age:apt:age:Encryption tool"
    "vidir:apt:moreutils:Bulk rename"
    "csvlens:cargo:csvlens:CSV previewer"
    "glow:cargo:glow:Markdown previewer"
    "chafa:apt:chafa:Image previewer"
    "sox:apt:sox:Audio previewer"
    "bat:cargo:bat:Cat with syntax highlighting" 
)

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Analyzing System Status ===${NC}\n"

missing_tools=()
apt_update_needed=true

# 1. Status Listing
for entry in "${tools[@]}"; do
    IFS=":" read -r bin mgr pkg desc <<< "$entry"
    
    if command -v "$bin" &> /dev/null; then
        echo -e "  ${GREEN}✔${NC} $bin is already installed."
    else
        echo -e "  ${RED}✘${NC} $bin is missing. (via $mgr)"
        missing_tools+=("$entry")
    fi
done

echo -e "\n${BLUE}=== Installation Phase ===${NC}\n"

if [ ${#missing_tools[@]} -eq 0 ]; then
    echo -e "${GREEN}All tools are already installed! Nothing to do.${NC}"
    exit 0
fi

# 2. Check for Cargo if any missing tools require it
cargo_needed=false
for entry in "${missing_tools[@]}"; do
    if [[ "$entry" == *":cargo:"* ]]; then
        cargo_needed=true
        break
    fi
done

if [ "$cargo_needed" = true ] && ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Cargo (Rust) is required for some of the modern tools but is not installed.${NC}"
    read -p "Would you like to install Cargo now? [y/N]: " install_cargo
    case "$install_cargo" in 
        [yY][eE][sS]|[yY]) 
            echo "Installing Rust/Cargo..."
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            # Source cargo env so it can be used immediately in this script
            source "$HOME/.cargo/env"
            ;;
        *)
            echo -e "${RED}Skipping Cargo. Rust-based tools will fail to install.${NC}"
            ;;
    esac
fi

# 3. Interactive Installation
for entry in "${missing_tools[@]}"; do
    IFS=":" read -r bin mgr pkg desc <<< "$entry"
    
    echo -e "\n${YELLOW}Tool:${NC} $bin"
    echo -e "${YELLOW}Desc:${NC} $desc"
    read -p "Install via $mgr? [y/N]: " choice
    
    case "$choice" in 
        [yY][eE][sS]|[yY]) 
            if [ "$mgr" == "apt" ]; then
                if [ "$apt_update_needed" = true ]; then
                    echo "Running apt update first..."
                    sudo apt-update -y
                    apt_update_needed=false
                fi
                echo "Installing $pkg via apt..."
                sudo apt-get install -y "$pkg"
            elif [ "$mgr" == "cargo" ]; then
                if command -v cargo &> /dev/null; then
                    echo "Installing $pkg via cargo (this may take a moment to compile)..."
                    cargo install "$pkg"
                else
                    echo -e "${RED}Cargo not found. Skipping $bin.${NC}"
                fi
            fi
            ;;
        *)
            echo "Skipping $bin."
            ;;
    esac
done

echo -e "\n${GREEN}Script finished!${NC} (Note: If you just installed Cargo, you may need to restart your terminal or run 'source ~/.cargo/env')"