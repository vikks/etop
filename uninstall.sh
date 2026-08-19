#!/usr/bin/env bash
# =============================================================================
# etop: Standalone Uninstaller
# Usage: curl -fsSL https://raw.githubusercontent.com/vikks/etop/main/uninstall.sh | sh
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [ -f "${SCRIPT_DIR}/install.sh" ]; then
    bash "${SCRIPT_DIR}/install.sh" --uninstall "$@"
else
    curl -fsSL https://raw.githubusercontent.com/vikks/etop/main/install.sh | bash -s -- --uninstall "$@"
fi
