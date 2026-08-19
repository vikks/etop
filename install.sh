#!/usr/bin/env bash
# =============================================================================
# etop: One-Line Universal Installer & Uninstaller for macOS
# Repository: https://github.com/vikks/etop
# Usage (Install):   curl -fsSL https://raw.githubusercontent.com/vikks/etop/main/install.sh | sh
# Usage (Uninstall): curl -fsSL https://raw.githubusercontent.com/vikks/etop/main/install.sh | sh -s -- --uninstall
# =============================================================================

set -euo pipefail

REPO="vikks/etop"
BIN_NAME="etop"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
DATA_DIR="${DATA_DIR:-$HOME/.local/share/etop}"

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# -----------------------------------------------------------------------------
# UNINSTALL MODE
# -----------------------------------------------------------------------------
if [[ "${1:-}" =~ ^(--uninstall|-u|uninstall)$ ]]; then
    printf "${CYAN}🗑️  Uninstalling etop...${NC}\n"

    REMOVED=0

    # 1. Remove binary from INSTALL_DIR
    if [ -f "${INSTALL_DIR}/${BIN_NAME}" ]; then
        rm -f "${INSTALL_DIR}/${BIN_NAME}"
        printf "${GREEN}✓ Removed binary:${NC} ${INSTALL_DIR}/${BIN_NAME}\n"
        REMOVED=1
    fi

    # Check alternative paths (/usr/local/bin, /opt/homebrew/bin)
    if [ -f "/usr/local/bin/${BIN_NAME}" ]; then
        rm -f "/usr/local/bin/${BIN_NAME}"
        printf "${GREEN}✓ Removed binary:${NC} /usr/local/bin/${BIN_NAME}\n"
        REMOVED=1
    fi

    # 2. Check for optional --purge flag to remove data directory
    if [[ "${2:-}" == "--purge" ]]; then
        if [ -d "$DATA_DIR" ]; then
            rm -rf "$DATA_DIR"
            printf "${GREEN}✓ Purged tombstone history data:${NC} ${DATA_DIR}\n"
        fi
    else
        if [ -d "$DATA_DIR" ]; then
            printf "${YELLOW}ℹ️  Preserved tombstone history archive at:${NC} ${DATA_DIR}\n"
            printf "   (Pass '--purge' to delete history data as well: 'install.sh --uninstall --purge')\n"
        fi
    fi

    if [ "$REMOVED" -eq 1 ]; then
        printf "\n${GREEN}✓ etop has been successfully uninstalled.${NC}\n"
    else
        printf "${YELLOW}⚠️  No etop binary was found in ${INSTALL_DIR}.${NC}\n"
    fi
    exit 0
fi

# -----------------------------------------------------------------------------
# INSTALL MODE
# -----------------------------------------------------------------------------
printf "${CYAN}⚡ Installing etop (Deterministic macOS Developer Ecosystem & Package Top)...${NC}\n"

# 1. Detect OS (macOS only)
OS="$(uname -s)"
if [ "$OS" != "Darwin" ]; then
    printf "${RED}❌ Error: etop is designed specifically for macOS (Darwin). Detected: ${OS}${NC}\n"
    exit 1
fi

# 2. Detect Architecture (arm64 vs x86_64)
ARCH="$(uname -m)"
case "$ARCH" in
    arm64|aarch64)
        TARGET="aarch64-apple-darwin"
        ;;
    x86_64)
        TARGET="x86_64-apple-darwin"
        ;;
    *)
        printf "${RED}❌ Error: Unsupported architecture: ${ARCH}${NC}\n"
        exit 1
        ;;
esac

# 3. Find latest release tag from GitHub API
printf "🔍 Checking latest release on GitHub...\n"
LATEST_TAG=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/' || true)

if [ -z "$LATEST_TAG" ]; then
    LATEST_TAG="v0.1.0"
fi

printf "📦 Downloading ${BIN_NAME} (${LATEST_TAG}) for ${TARGET}...\n"
ARCHIVE_NAME="${BIN_NAME}-${LATEST_TAG}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${ARCHIVE_NAME}"

TMP_DIR="$(mktemp -d)"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if ! curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}"; then
    # Fallback to direct binary build from cargo if release assets are still building
    printf "${YELLOW}⚠️ Release binary not found at ${DOWNLOAD_URL}.${NC}\n"
    printf "Falling back to installing via cargo if available...\n"
    if command -v cargo >/dev/null 2>&1; then
        cargo install --git "https://github.com/${REPO}.git" etop
        printf "${GREEN}✓ Successfully installed etop via Cargo!${NC}\n"
        exit 0
    else
        printf "${RED}❌ Please download etop from https://github.com/${REPO}/releases or build with cargo.${NC}\n"
        exit 1
    fi
fi

# 4. Extract Archive
tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "$TMP_DIR"

# 5. Create Install Directory and Copy Binary
mkdir -p "$INSTALL_DIR"
cp "${TMP_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
chmod +x "${INSTALL_DIR}/${BIN_NAME}"

printf "\n${GREEN}✓ Successfully installed ${BIN_NAME} to ${INSTALL_DIR}/${BIN_NAME}!${NC}\n\n"

# 6. Check if INSTALL_DIR is in PATH
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    printf "${YELLOW}⚠️  Note: ${INSTALL_DIR} is not in your \$PATH.${NC}\n"
    printf "Add it to your shell configuration (e.g. ~/.zshrc or ~/.bash_profile):\n"
    printf "   ${CYAN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}\n\n"
fi

printf "🚀 Run '${CYAN}etop${NC}' to launch the interactive TUI dashboard!\n"
