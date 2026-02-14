#!/usr/bin/env bash
BIN="$(dirname "$0")/../bin/exo-self"
if [ ! -x "$BIN" ]; then
    # Binary missing — emit a helpful message instead of failing silently
    SETUP="$(dirname "$0")/../setup.sh"
    cat <<EOF
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"## Exo-Self: Setup Required\n\nThe exo-self binary hasn't been built yet. Run:\n\n\`\`\`bash\n${SETUP}\n\`\`\`\n\nThis compiles the Rust binary and configures the plugin. Only needed once per machine (and after upgrades)."}}
EOF
    exit 0
fi
exec "$BIN" session-start
