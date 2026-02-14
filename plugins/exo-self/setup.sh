#!/usr/bin/env bash
# setup.sh — Post-install setup for exo-self
#
# Run AFTER installing via native marketplace commands:
#   claude plugin marketplace add lexicone42/exo-self
#   claude plugin install exo-self@exo-self
#
# This script handles what the marketplace can't:
#   - Build the Rust binary
#   - Create runtime directories and default config
#   - Configure statusline and permissions
#
# Prerequisites: cargo (Rust toolchain), jq

set -euo pipefail

PLUGIN_NAME="exo-self"
MARKETPLACE="exo-self"
PLUGIN_KEY="${PLUGIN_NAME}@${MARKETPLACE}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLAUDE_DIR="$HOME/.claude"
EXO_DIR="$CLAUDE_DIR/exo-self"
SETTINGS_JSON="$CLAUDE_DIR/settings.json"
INSTALLED_JSON="$CLAUDE_DIR/plugins/installed_plugins.json"
KNOWN_MKT_JSON="$CLAUDE_DIR/plugins/known_marketplaces.json"

# --- Prerequisites ---
for cmd in cargo jq; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: $cmd is required but not found."
        exit 1
    fi
done

echo "=== exo-self setup ==="

# --- 1. Build Rust binary ---
echo "1. Building binary..."
(cd "$SCRIPT_DIR" && cargo build --release --quiet 2>&1)

# Cargo may output to a workspace-level target dir; find the binary
BINARY=""
for CANDIDATE in \
    "$SCRIPT_DIR/target/release/exo-self" \
    "$SCRIPT_DIR/../../target/release/exo-self"; do
    if [ -f "$CANDIDATE" ]; then
        BINARY="$(realpath "$CANDIDATE")"
        break
    fi
done
if [ -z "$BINARY" ]; then
    echo "ERROR: Binary not found after build. Check cargo output."
    exit 1
fi

mkdir -p "$SCRIPT_DIR/bin"
cp "$BINARY" "$SCRIPT_DIR/bin/exo-self"
chmod +x "$SCRIPT_DIR/bin/exo-self"
echo "   $(du -h "$SCRIPT_DIR/bin/exo-self" | cut -f1)"

# --- 2. Sync binary to marketplace clone and cache ---
echo "2. Syncing binary..."
LINK_TARGET="$SCRIPT_DIR/bin/exo-self"

# Marketplace clone
MKT_DIR=$(jq -r ".[\"$MARKETPLACE\"].installLocation // empty" "$KNOWN_MKT_JSON" 2>/dev/null || true)
if [ -n "$MKT_DIR" ] && [ -d "$MKT_DIR/plugins/$PLUGIN_NAME" ]; then
    MKT_PLUGIN_DIR="$MKT_DIR/plugins/$PLUGIN_NAME"
    if [ "$(realpath "$SCRIPT_DIR")" != "$(realpath "$MKT_PLUGIN_DIR")" ]; then
        mkdir -p "$MKT_PLUGIN_DIR/bin"
        cp "$SCRIPT_DIR/bin/exo-self" "$MKT_PLUGIN_DIR/bin/exo-self"
        echo "   -> marketplace clone"
    fi
    LINK_TARGET="$MKT_PLUGIN_DIR/bin/exo-self"
fi

# Cache (where CLAUDE_PLUGIN_ROOT resolves to)
CACHE_DIR=$(jq -r ".plugins[\"$PLUGIN_KEY\"][0].installPath // empty" "$INSTALLED_JSON" 2>/dev/null || true)
if [ -n "$CACHE_DIR" ] && [ -d "$CACHE_DIR" ]; then
    mkdir -p "$CACHE_DIR/bin"
    cp "$SCRIPT_DIR/bin/exo-self" "$CACHE_DIR/bin/exo-self"
    echo "   -> cache"
fi

# --- 3. Runtime directories ---
echo "3. Creating runtime directories..."
mkdir -p "$EXO_DIR"/{reflections,per-project,sessions}

# --- 4. Default config ---
if [ ! -f "$EXO_DIR/config.json" ]; then
    echo "4. Creating default config..."
    cat > "$EXO_DIR/config.json" << 'EOF'
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
EOF
else
    echo "4. Config exists."
fi

# --- 5. Statusline ---
echo "5. Installing statusline..."
if [ -f "$SCRIPT_DIR/statusline.sh" ]; then
    cp "$SCRIPT_DIR/statusline.sh" "$CLAUDE_DIR/statusline.sh"
    chmod +x "$CLAUDE_DIR/statusline.sh"
    mkdir -p "$CLAUDE_DIR/bin"
    ln -sf "$LINK_TARGET" "$CLAUDE_DIR/bin/exo-self"
    echo "   -> ~/.claude/statusline.sh"
    echo "   -> ~/.claude/bin/exo-self"
fi

# --- 6. Settings (permissions + statusline) ---
echo "6. Updating settings.json..."
[ -f "$SETTINGS_JSON" ] || echo '{}' > "$SETTINGS_JSON"

NEEDED_ALLOWS=(
    "Read($EXO_DIR/**)"
    "Write($EXO_DIR/**)"
    "Edit($EXO_DIR/**)"
    "Skill(exo-self:context-budget)"
    "Skill(exo-self:exo)"
    "Skill(exo-self:interests)"
    "Skill(exo-self:reflect)"
    "Skill(exo-self:self-reflection)"
)

ADDED=0
for RULE in "${NEEDED_ALLOWS[@]}"; do
    if ! jq -e ".permissions.allow // [] | index(\"$RULE\")" "$SETTINGS_JSON" &>/dev/null; then
        jq ".permissions.allow = ((.permissions.allow // []) + [\"$RULE\"])" "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
        ADDED=$((ADDED + 1))
    fi
done
[ "$ADDED" -gt 0 ] && echo "   $ADDED permission(s) added" || echo "   Permissions already set"

EXPECTED='{"type":"command","command":"~/.claude/statusline.sh","padding":0}'
CURRENT=$(jq -c '.statusLine // {}' "$SETTINGS_JSON" 2>/dev/null)
if [ "$CURRENT" != "$EXPECTED" ]; then
    jq '.statusLine = {"type":"command","command":"~/.claude/statusline.sh","padding":0}' "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
    echo "   Statusline configured"
else
    echo "   Statusline already set"
fi

echo ""
echo "=== Done. Restart Claude Code to activate. ==="
