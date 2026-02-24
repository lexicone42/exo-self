#!/usr/bin/env bash
# Shared guard for hook handlers: validates the binary exists and supports
# the requested subcommand. Falls back to no-op JSON if stale/missing.
#
# Usage (from a hook handler):
#   SUBCMD="failure-tracker"
#   source "$(dirname "$0")/_common.sh"
#   exec "$BIN" "$SUBCMD"

HANDLERS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HANDLERS_DIR/../bin/exo-self"

if [ ! -x "$BIN" ]; then
    echo '{}'
    exit 0
fi

# Verify the binary knows about the requested subcommand
if [ -n "$SUBCMD" ] && ! "$BIN" help 2>&1 | grep -q "  ${SUBCMD} "; then
    echo '{}'
    exit 0
fi
