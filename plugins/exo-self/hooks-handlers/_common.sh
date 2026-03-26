#!/usr/bin/env bash
# Shared guard for hook handlers: auto-builds if binary is missing/stale,
# then validates the requested subcommand. Falls back to no-op JSON if
# building fails or subcommand is unsupported.
#
# Usage (from a hook handler):
#   SUBCMD="failure-tracker"
#   source "$(dirname "$0")/_common.sh"
#   exec "$BIN" "$SUBCMD"

# Platform-aware binary selection: macOS and Linux (minimal sandbox) may
# share ~/.claude via bind mount but need different binaries.
_PLAT="$(uname -s)-$(uname -m)"
case "$_PLAT" in
    Linux-x86_64)  _SUFFIX="-linux-x64" ;;
    Linux-aarch64) _SUFFIX="-linux-arm64" ;;
    *)             _SUFFIX="" ;;  # macOS or unknown — use default binary
esac

BIN="$HOME/.claude/bin/exo-self${_SUFFIX}"
# Fallback to unsuffixed binary (backward compat, single-platform setups)
[ ! -x "$BIN" ] && BIN="$HOME/.claude/bin/exo-self"

SETUP="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/setup.sh"

# Auto-build if binary is missing and cargo is available
if [ ! -x "$BIN" ] && command -v cargo &>/dev/null && [ -x "$SETUP" ]; then
    "$SETUP" >&2 2>&1
    # Re-check after build with platform suffix
    BIN="$HOME/.claude/bin/exo-self${_SUFFIX}"
    [ ! -x "$BIN" ] && BIN="$HOME/.claude/bin/exo-self"
fi

if [ ! -x "$BIN" ]; then
    echo '{}'
    exit 0
fi

# Check subcommand support via help output
if [ -n "$SUBCMD" ]; then
    "$BIN" help 2>&1 | grep -q "  ${SUBCMD}" || { echo '{}'; exit 0; }
fi
