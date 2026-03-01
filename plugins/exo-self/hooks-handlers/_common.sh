#!/usr/bin/env bash
# Shared guard for hook handlers: validates the binary exists and supports
# the requested subcommand. Falls back to no-op JSON if stale/missing.
#
# Uses .manifest (written by setup.sh) for O(1) file check instead of
# spawning a subprocess on every hook invocation.
#
# Usage (from a hook handler):
#   SUBCMD="failure-tracker"
#   source "$(dirname "$0")/_common.sh"
#   exec "$BIN" "$SUBCMD"

HANDLERS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Use the stable symlink (set by setup.sh) rather than the relative cache path.
# The cache dir changes on every plugin SHA update, but setup.sh only runs on
# explicit install/update — so the relative path goes stale between updates.
BIN="$HOME/.claude/bin/exo-self"
MANIFEST="$(dirname "$(readlink -f "$BIN" 2>/dev/null || echo "$BIN")")/.manifest"

if [ ! -x "$BIN" ]; then
    echo '{}'
    exit 0
fi

# Check manifest for subcommand support (no subprocess spawn)
if [ -n "$SUBCMD" ]; then
    if [ -f "$MANIFEST" ]; then
        grep -qx "$SUBCMD" "$MANIFEST" || { echo '{}'; exit 0; }
    else
        # No manifest — fall back to subprocess check (pre-upgrade installs)
        "$BIN" help 2>&1 | grep -q "  ${SUBCMD} " || { echo '{}'; exit 0; }
    fi
fi
