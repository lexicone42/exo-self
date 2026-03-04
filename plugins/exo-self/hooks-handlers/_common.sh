#!/usr/bin/env bash
# Shared guard for hook handlers: auto-builds if binary is missing/stale,
# then validates the requested subcommand. Falls back to no-op JSON if
# building fails or subcommand is unsupported.
#
# Usage (from a hook handler):
#   SUBCMD="failure-tracker"
#   source "$(dirname "$0")/_common.sh"
#   exec "$BIN" "$SUBCMD"

BIN="$HOME/.claude/bin/exo-self"
SETUP="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/setup.sh"

# Auto-build if binary is missing and cargo is available
if [ ! -x "$BIN" ] && command -v cargo &>/dev/null && [ -x "$SETUP" ]; then
    "$SETUP" >&2 2>&1
fi

if [ ! -x "$BIN" ]; then
    echo '{}'
    exit 0
fi

# Check subcommand support via help output
if [ -n "$SUBCMD" ]; then
    "$BIN" help 2>&1 | grep -q "  ${SUBCMD}" || { echo '{}'; exit 0; }
fi
