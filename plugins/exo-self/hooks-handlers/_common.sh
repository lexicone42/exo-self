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
BIN="$HANDLERS_DIR/../bin/exo-self"
MANIFEST="$HANDLERS_DIR/../bin/.manifest"

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
