#!/usr/bin/env bash
# setup.sh — Build exo-self binary after marketplace install/update
#
# Usage:
#   claude plugin marketplace add lexicone42/exo-self   # first time
#   claude plugin marketplace update exo-self            # pull latest
#   ~/.claude/plugins/marketplaces/exo-self/plugins/exo-self/setup.sh
#
# The marketplace handles: git pull, cache sync, metadata, enabling.
# This script handles: building the Rust binary and first-time runtime setup.
#
# Prerequisites: cargo, jq

set -euo pipefail

PLUGIN_NAME="exo-self"
MARKETPLACE="exo-self"
PLUGIN_KEY="${PLUGIN_NAME}@${MARKETPLACE}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLAUDE_DIR="$HOME/.claude"
EXO_DIR="$CLAUDE_DIR/exo-self"
SETTINGS_JSON="$CLAUDE_DIR/settings.json"
INSTALLED_JSON="$CLAUDE_DIR/plugins/installed_plugins.json"
PLUGIN_JSON="$SCRIPT_DIR/.claude-plugin/plugin.json"

# --- Prerequisites ---
for cmd in cargo jq; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: $cmd is required but not found."
        exit 1
    fi
done

VERSION=$(jq -r '.version' "$PLUGIN_JSON")

# Resolve cache directory from installed_plugins.json (set by marketplace)
CACHE_DIR=$(jq -r ".plugins[\"$PLUGIN_KEY\"][0].installPath // empty" "$INSTALLED_JSON" 2>/dev/null || true)
if [ -z "$CACHE_DIR" ] || [ ! -d "$CACHE_DIR" ]; then
    echo "ERROR: Plugin not found in installed_plugins.json."
    echo "Run first: claude plugin marketplace add lexicone42/exo-self"
    exit 1
fi

echo "=== exo-self setup (v${VERSION}) ==="

# --- 1. Build binary ---
echo "1. Building binary..."
(cd "$SCRIPT_DIR" && cargo build --release --quiet 2>&1)

# Cargo may output to workspace-level target dir; prefer newest binary
BINARY=""
for CANDIDATE in \
    "$SCRIPT_DIR/../../target/release/exo-self" \
    "$SCRIPT_DIR/target/release/exo-self"; do
    if [ -f "$CANDIDATE" ]; then
        BINARY="$(cd "$(dirname "$CANDIDATE")" && pwd)/$(basename "$CANDIDATE")"
        break
    fi
done
if [ -z "$BINARY" ]; then
    echo "ERROR: Binary not found after build."
    exit 1
fi
echo "   $(du -h "$BINARY" | cut -f1)"

# --- 2. Install binary to cache + symlink ---
echo "2. Installing binary..."
mkdir -p "$CACHE_DIR/bin"
cp "$BINARY" "$CACHE_DIR/bin/exo-self"
chmod +x "$CACHE_DIR/bin/exo-self"
echo "   -> cache"

mkdir -p "$CLAUDE_DIR/bin"
ln -sf "$CACHE_DIR/bin/exo-self" "$CLAUDE_DIR/bin/exo-self"
echo "   -> ~/.claude/bin/exo-self"

# --- 3. Runtime setup (first-time, idempotent) ---
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

# --- 4. Permissions (idempotent) ---
echo "4. Permissions..."
[ -f "$SETTINGS_JSON" ] || echo '{}' > "$SETTINGS_JSON"

NEEDED_ALLOWS=(
    "Read($HOME/.claude/**)"
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
[ "$ADDED" -gt 0 ] && echo "   $ADDED permission(s) added" || echo "   ok"

EXPECTED='{"type":"command","command":"~/.claude/statusline.sh","padding":0}'
CURRENT=$(jq -c '.statusLine // {}' "$SETTINGS_JSON" 2>/dev/null)
if [ "$CURRENT" != "$EXPECTED" ]; then
    jq '.statusLine = {"type":"command","command":"~/.claude/statusline.sh","padding":0}' "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
fi

echo ""
echo "=== Done (v${VERSION}). Restart Claude Code to activate. ==="
