#!/usr/bin/env bash
HANDLERS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$HANDLERS_DIR/../bin/exo-self"
MANIFEST="$HANDLERS_DIR/../bin/.manifest"

if [ ! -x "$BIN" ]; then
    # Binary missing — emit a helpful message instead of failing silently
    SETUP="$(dirname "$0")/../setup.sh"
    cat <<EOF
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"## Exo-Self: Setup Required\n\nThe exo-self binary hasn't been built yet. Run:\n\n\`\`\`bash\n${SETUP}\n\`\`\`\n\nThis compiles the Rust binary and configures the plugin. Only needed once per machine (and after upgrades)."}}
EOF
    exit 0
fi

# Verify the binary supports session-start (stale binary detection)
# Uses manifest file if available, falls back to subprocess check
STALE=false
if [ -f "$MANIFEST" ]; then
    grep -qx "session-start" "$MANIFEST" || STALE=true
else
    "$BIN" help 2>&1 | grep -q "  session-start " || STALE=true
fi

if [ "$STALE" = true ]; then
    SETUP="$(dirname "$0")/../setup.sh"
    cat <<EOF
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"## Exo-Self: Binary Outdated\n\nThe exo-self binary is missing expected subcommands. Rebuild with:\n\n\`\`\`bash\n${SETUP}\n\`\`\`"}}
EOF
    exit 0
fi

exec "$BIN" session-start
