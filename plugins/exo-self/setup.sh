#!/usr/bin/env bash
# setup.sh — Build exo-self after marketplace install/update
#
# Usage:
#   claude plugin marketplace add lexicone42/exo-self   # first time
#   claude plugin marketplace update exo-self            # pull latest
#   ~/.claude/plugins/marketplaces/exo-self/plugins/exo-self/setup.sh
#
# The marketplace handles: git pull, cache sync, metadata, enabling.
# This script handles: building the Rust binary and first-time runtime setup.
#
# All tools (preflight, patchpath, reflect) are now subcommands of the
# single exo-self binary. No more separate binaries or fragile symlinks.
#
# Prerequisites: cargo

set -euo pipefail

PLUGIN_NAME="exo-self"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLAUDE_DIR="$HOME/.claude"
EXO_DIR="$CLAUDE_DIR/exo-self"
BIN_DIR="$CLAUDE_DIR/bin"

# --- Prerequisites ---
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo is required but not found."
    echo "Install Rust: https://rustup.rs"
    exit 1
fi

# --- 1. Detect workspace layout ---
# Dev repo:  setup.sh is at plugins/exo-self/, workspace Cargo.toml at ../../
# Cache:     setup.sh is at the cache root, Cargo.toml beside it (no parent workspace)
# Check for workspace root FIRST — the plugin always has its own Cargo.toml,
# so checking SCRIPT_DIR first would always match and miss the workspace.
if [ -f "$SCRIPT_DIR/../../Cargo.toml" ]; then
    WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
elif [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
    WORKSPACE_ROOT="$SCRIPT_DIR"
else
    echo "ERROR: Cannot find Cargo.toml in $SCRIPT_DIR or $SCRIPT_DIR/../.."
    exit 1
fi

echo "=== exo-self setup ==="
echo "  workspace: $WORKSPACE_ROOT"

# --- 2. Build ---
echo "1. Building..."
(cd "$WORKSPACE_ROOT" && cargo build --release --quiet 2>&1)

# Find the binary — could be in workspace target or local target
if [ -f "$WORKSPACE_ROOT/target/release/exo-self" ]; then
    BUILT_BIN="$WORKSPACE_ROOT/target/release/exo-self"
else
    echo "ERROR: Binary not found at $WORKSPACE_ROOT/target/release/exo-self"
    exit 1
fi

# --- 3. Install binary (copy, not symlink — survives cache rotation) ---
echo "2. Installing..."
mkdir -p "$BIN_DIR"
rm -f "$BIN_DIR/exo-self"
cp "$BUILT_BIN" "$BIN_DIR/exo-self"
chmod +x "$BIN_DIR/exo-self"
echo "   exo-self $(du -h "$BIN_DIR/exo-self" | cut -f1) -> $BIN_DIR/exo-self"

# Create wrapper scripts for backward compatibility (pre-commit, scripts, etc.)
# Remove old symlinks/files first to avoid "Text file busy" on active executables.
for TOOL in preflight patchpath reflect; do
    rm -f "$BIN_DIR/$TOOL"
    cat > "$BIN_DIR/$TOOL" << WRAPPER
#!/bin/sh
exec "\$(dirname "\$0")/exo-self" $TOOL "\$@"
WRAPPER
    chmod +x "$BIN_DIR/$TOOL"
done
echo "   wrappers: preflight, patchpath, reflect"

# --- 4. Runtime setup (first-time, idempotent) ---
echo "3. Runtime setup..."
mkdir -p "$EXO_DIR"/{reflections,per-project,sessions,handoffs}

if [ ! -f "$EXO_DIR/config.json" ]; then
    cat > "$EXO_DIR/config.json" << 'CONFIGEOF'
{
  "estimated_max_chars": 800000,
  "nudge_threshold": 0.40,
  "checkin_threshold": 0.60,
  "reserve_threshold": 0.80,
  "max_journal_chars": 1500,
  "max_journal_entries": 2,
  "max_interests_items": 5,
  "max_sparks_display": 5
}
CONFIGEOF
    echo "   config created"
fi

if [ -f "$SCRIPT_DIR/statusline.sh" ]; then
    cp "$SCRIPT_DIR/statusline.sh" "$CLAUDE_DIR/statusline.sh"
    chmod +x "$CLAUDE_DIR/statusline.sh"
fi

# --- 5. Permissions (idempotent) ---
echo "4. Permissions..."
SETTINGS_JSON="$CLAUDE_DIR/settings.json"
[ -f "$SETTINGS_JSON" ] || echo '{}' > "$SETTINGS_JSON"

NEEDED_ALLOWS=(
    "Read($HOME/.claude/**)"
    "Write($EXO_DIR/**)"
    "Edit($EXO_DIR/**)"
    "Skill(exo-self:exo)"
    "Skill(exo-self:interests)"
    "Skill(exo-self:self-reflection)"
)

ADDED=0
for RULE in "${NEEDED_ALLOWS[@]}"; do
    if ! jq -e ".permissions.allow // [] | index(\"$RULE\")" "$SETTINGS_JSON" &>/dev/null; then
        if command -v jq &>/dev/null; then
            jq ".permissions.allow = ((.permissions.allow // []) + [\"$RULE\"])" "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
            ADDED=$((ADDED + 1))
        fi
    fi
done
[ "$ADDED" -gt 0 ] && echo "   $ADDED permission(s) added" || echo "   ok"

if command -v jq &>/dev/null; then
    EXPECTED='{"type":"command","command":"~/.claude/statusline.sh","padding":0}'
    CURRENT=$(jq -c '.statusLine // {}' "$SETTINGS_JSON" 2>/dev/null)
    if [ "$CURRENT" != "$EXPECTED" ]; then
        jq '.statusLine = {"type":"command","command":"~/.claude/statusline.sh","padding":0}' "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
    fi
fi

echo ""
echo "=== Done. Restart Claude Code to activate. ==="
