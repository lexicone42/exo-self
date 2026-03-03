#!/usr/bin/env bash
# Shared guard for hook handlers: validates the binary exists and supports
# the requested subcommand. Falls back to no-op JSON if stale/missing.
#
# Usage (from a hook handler):
#   SUBCMD="failure-tracker"
#   source "$(dirname "$0")/_common.sh"
#   exec "$BIN" "$SUBCMD"

BIN="$HOME/.claude/bin/exo-self"

if [ ! -x "$BIN" ]; then
    echo '{}'
    exit 0
fi

# Check subcommand support via help output (no manifest file needed)
if [ -n "$SUBCMD" ]; then
    "$BIN" help 2>&1 | grep -q "  ${SUBCMD}" || { echo '{}'; exit 0; }
fi
