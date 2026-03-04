#!/usr/bin/env bash
# Session-start hook: auto-builds if binary is missing/stale (fixes #5).
SETUP="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/setup.sh"
BIN="$HOME/.claude/bin/exo-self"

auto_setup() {
    # Auto-run setup.sh if cargo is available; fall back to manual message if not
    if command -v cargo &>/dev/null && [ -x "$SETUP" ]; then
        "$SETUP" >&2 2>&1
        [ -x "$BIN" ] && return 0
    fi
    # Can't auto-build — tell the user
    cat <<EOF
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"## Exo-Self: Setup Required\n\nThe exo-self binary needs to be built. Run:\n\n\`\`\`bash\n${SETUP}\n\`\`\`\n\nRequires cargo (https://rustup.rs)."}}
EOF
    exit 0
}

# Binary missing → auto-build
[ -x "$BIN" ] || auto_setup

# Binary stale (missing session-start subcommand) → auto-build
"$BIN" help 2>&1 | grep -q "  session-start " || auto_setup

exec "$BIN" session-start
