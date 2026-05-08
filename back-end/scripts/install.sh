#!/bin/sh
set -eu

GITHUB_REPO="malboxapp/malbox"
BINARY_NAME="malbox"

main() {
    check_os
    detect_arch
    detect_install_dir

    printf "Installing Malbox CLI...\n"
    printf "  Architecture: %s\n" "$ARCH"
    printf "  Install to:   %s\n" "$INSTALL_DIR/$BINARY_NAME"
    printf "\n"

    if [ -f "$INSTALL_DIR/$BINARY_NAME" ] && [ "${MALBOX_FORCE:-}" != "1" ]; then
        printf "Malbox CLI already exists at %s/%s\n" "$INSTALL_DIR" "$BINARY_NAME"
        printf "Overwrite? [y/N] "
        read -r answer
        case "$answer" in
            [yY]|[yY][eE][sS]) ;;
            *) printf "Installation cancelled.\n"; exit 0 ;;
        esac
    fi

    fetch_latest_release
    download_binary
    verify_checksum
    install_binary

    printf "\n"
    printf "Malbox CLI installed successfully!\n"
    printf "Run 'malbox install' to set up Malbox.\n"
}

check_os() {
    OS="$(uname -s)"
    case "$OS" in
        Linux) ;;
        *)
            printf "Error: Malbox currently only supports Linux (got %s)\n" "$OS" >&2
            exit 1
            ;;
    esac
}

detect_arch() {
    UNAME_ARCH="$(uname -m)"
    case "$UNAME_ARCH" in
        x86_64|amd64) ARCH="x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) ARCH="aarch64-unknown-linux-gnu" ;;
        *)
            printf "Error: unsupported architecture: %s\n" "$UNAME_ARCH" >&2
            exit 1
            ;;
    esac
}

detect_install_dir() {
    if [ "$(id -u)" = "0" ]; then
        INSTALL_DIR="/usr/local/bin"
    elif printf "%s" "$PATH" | tr ':' '\n' | grep -qx "$HOME/.local/bin"; then
        INSTALL_DIR="$HOME/.local/bin"
    else
        INSTALL_DIR="$HOME/.local/bin"
        printf "Note: %s is not on your PATH.\n" "$INSTALL_DIR"
        printf "Add it with: export PATH=\"\$HOME/.local/bin:\$PATH\"\n\n"
    fi
    mkdir -p "$INSTALL_DIR"
}

fetch_latest_release() {
    printf "Fetching latest release...\n"
    RELEASE_JSON="$(curl -sSfL "https://api.github.com/repos/$GITHUB_REPO/releases/latest")"
    TAG="$(printf "%s" "$RELEASE_JSON" | grep '"tag_name"' | head -1 | sed 's/.*: *"//;s/".*//')"

    if [ -z "$TAG" ]; then
        printf "Error: could not determine latest release\n" >&2
        exit 1
    fi

    printf "  Latest version: %s\n" "$TAG"
}

download_binary() {
    ASSET_NAME="malbox-cli-${ARCH}.tar.gz"
    DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$TAG/$ASSET_NAME"
    CHECKSUM_URL="https://github.com/$GITHUB_REPO/releases/download/$TAG/$ASSET_NAME.sha256"

    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TMP_DIR"' EXIT

    printf "Downloading %s...\n" "$ASSET_NAME"
    curl -sSfL -o "$TMP_DIR/$ASSET_NAME" "$DOWNLOAD_URL"
    curl -sSfL -o "$TMP_DIR/$ASSET_NAME.sha256" "$CHECKSUM_URL"
}

verify_checksum() {
    printf "Verifying checksum...\n"
    cd "$TMP_DIR"

    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum -c "$ASSET_NAME.sha256"
    elif command -v shasum > /dev/null 2>&1; then
        shasum -a 256 -c "$ASSET_NAME.sha256"
    else
        printf "Warning: no sha256 tool found, skipping verification\n" >&2
    fi

    cd - > /dev/null
}

install_binary() {
    printf "Extracting...\n"
    tar -xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"
    mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
}

main
