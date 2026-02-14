#!/usr/bin/env bash
# setup.sh — Install or upgrade exo-self
#
# Works for both fresh install and upgrades:
#   claude plugin marketplace add lexicone42/exo-self   # first time only
#   claude plugin marketplace update exo-self            # pull latest
#   ~/.claude/plugins/marketplaces/exo-self/plugins/exo-self/setup.sh
#
# What this does:
#   1. Build the Rust binary (or download pre-built if available)
#   2. Sync to plugin cache with correct version
#   3. Update installed_plugins.json
#   4. Create runtime directories and default config
#   5. Configure statusline and permissions
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
PLUGIN_JSON="$SCRIPT_DIR/.claude-plugin/plugin.json"

# --- Prerequisites ---
for cmd in cargo jq; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: $cmd is required but not found."
        exit 1
    fi
done

# --- Read version ---
NEW_VERSION=$(jq -r '.version' "$PLUGIN_JSON")
OLD_VERSION=$(jq -r ".plugins[\"$PLUGIN_KEY\"][0].version // empty" "$INSTALLED_JSON" 2>/dev/null || true)

if [ -n "$OLD_VERSION" ] && [ "$OLD_VERSION" = "$NEW_VERSION" ]; then
    echo "=== exo-self setup (rebuild v${NEW_VERSION}) ==="
else
    if [ -n "$OLD_VERSION" ]; then
        echo "=== exo-self upgrade (v${OLD_VERSION} → v${NEW_VERSION}) ==="
    else
        echo "=== exo-self install (v${NEW_VERSION}) ==="
    fi
fi

# --- 1. Build Rust binary ---
echo "1. Building binary..."
(cd "$SCRIPT_DIR" && cargo build --release --quiet 2>&1)

# Cargo may output to a workspace-level target dir; find the binary
BINARY=""
for CANDIDATE in \
    "$SCRIPT_DIR/target/release/exo-self" \
    "$SCRIPT_DIR/../../target/release/exo-self"; do
    if [ -f "$CANDIDATE" ]; then
        BINARY="$(cd "$(dirname "$CANDIDATE")" && pwd)/$(basename "$CANDIDATE")"
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

# --- 2. Sync to marketplace clone ---
echo "2. Syncing..."
MKT_DIR=$(jq -r ".[\"$MARKETPLACE\"].installLocation // empty" "$KNOWN_MKT_JSON" 2>/dev/null || true)
if [ -n "$MKT_DIR" ] && [ -d "$MKT_DIR/plugins/$PLUGIN_NAME" ]; then
    MKT_PLUGIN_DIR="$MKT_DIR/plugins/$PLUGIN_NAME"
    if [ "$(cd "$SCRIPT_DIR" && pwd)" != "$(cd "$MKT_PLUGIN_DIR" && pwd)" ]; then
        mkdir -p "$MKT_PLUGIN_DIR/bin"
        cp "$SCRIPT_DIR/bin/exo-self" "$MKT_PLUGIN_DIR/bin/exo-self"
        echo "   -> marketplace clone"
    fi
fi

# --- 3. Update plugin cache and metadata ---
echo "3. Updating plugin cache..."
CACHE_BASE="$CLAUDE_DIR/plugins/cache/$MARKETPLACE/$PLUGIN_NAME"
CACHE_DIR="$CACHE_BASE/$NEW_VERSION"

# Create new version cache dir
mkdir -p "$CACHE_DIR"

# Sync plugin files to cache (exclude build artifacts)
rsync -a \
    --exclude='target/' \
    --exclude='.git/' \
    --exclude='*.o' \
    --exclude='*.d' \
    "$SCRIPT_DIR/" "$CACHE_DIR/"

# Ensure binary is in cache
mkdir -p "$CACHE_DIR/bin"
cp "$SCRIPT_DIR/bin/exo-self" "$CACHE_DIR/bin/exo-self"
echo "   -> cache ($NEW_VERSION)"

# Remove old version cache if upgrading
if [ -n "$OLD_VERSION" ] && [ "$OLD_VERSION" != "$NEW_VERSION" ] && [ -d "$CACHE_BASE/$OLD_VERSION" ]; then
    rm -rf "$CACHE_BASE/$OLD_VERSION"
    echo "   removed old cache ($OLD_VERSION)"
fi

# --- 4. Update installed_plugins.json ---
echo "4. Updating installed_plugins.json..."
[ -f "$INSTALLED_JSON" ] || echo '{"plugins":{}}' > "$INSTALLED_JSON"

# Get git SHA if available
GIT_SHA=""
if command -v git &>/dev/null; then
    # Try marketplace clone first, then script dir
    for GIT_DIR in "$MKT_DIR" "$SCRIPT_DIR" "$SCRIPT_DIR/../.."; do
        if [ -n "$GIT_DIR" ] && [ -d "$GIT_DIR/.git" ]; then
            GIT_SHA=$(git -C "$GIT_DIR" rev-parse HEAD 2>/dev/null || true)
            break
        fi
    done
fi

# Preserve original installedAt or set new one
INSTALLED_AT=$(jq -r ".plugins[\"$PLUGIN_KEY\"][0].installedAt // empty" "$INSTALLED_JSON" 2>/dev/null || true)
[ -z "$INSTALLED_AT" ] && INSTALLED_AT=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")

NOW=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")

jq --arg key "$PLUGIN_KEY" \
   --arg ver "$NEW_VERSION" \
   --arg path "$CACHE_DIR" \
   --arg sha "$GIT_SHA" \
   --arg installed "$INSTALLED_AT" \
   --arg updated "$NOW" \
   '.plugins[$key] = [{
     "scope": "user",
     "installPath": $path,
     "version": $ver,
     "installedAt": $installed,
     "lastUpdated": $updated,
     "gitCommitSha": $sha
   }]' "$INSTALLED_JSON" > "${INSTALLED_JSON}.tmp" && mv "${INSTALLED_JSON}.tmp" "$INSTALLED_JSON"
echo "   version=$NEW_VERSION sha=${GIT_SHA:0:8}"

# --- 5. Enable plugin ---
echo "5. Enabling plugin..."
if ! jq -e ".enabledPlugins[\"$PLUGIN_KEY\"]" "$SETTINGS_JSON" &>/dev/null 2>&1; then
    [ -f "$SETTINGS_JSON" ] || echo '{}' > "$SETTINGS_JSON"
    jq ".enabledPlugins[\"$PLUGIN_KEY\"] = true" "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
    echo "   enabled"
else
    echo "   already enabled"
fi

# --- 6. Runtime directories ---
echo "6. Runtime directories..."
mkdir -p "$EXO_DIR"/{reflections,per-project,sessions,handoffs}

# --- 7. Default config ---
if [ ! -f "$EXO_DIR/config.json" ]; then
    echo "7. Creating default config..."
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
    echo "7. Config exists."
fi

# --- 8. Statusline + bin symlink ---
echo "8. Statusline..."
if [ -f "$SCRIPT_DIR/statusline.sh" ]; then
    cp "$SCRIPT_DIR/statusline.sh" "$CLAUDE_DIR/statusline.sh"
    chmod +x "$CLAUDE_DIR/statusline.sh"
    mkdir -p "$CLAUDE_DIR/bin"
    ln -sf "$CACHE_DIR/bin/exo-self" "$CLAUDE_DIR/bin/exo-self"
    echo "   -> ~/.claude/statusline.sh"
    echo "   -> ~/.claude/bin/exo-self -> cache"
fi

# --- 9. Permissions ---
echo "9. Permissions..."
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
[ "$ADDED" -gt 0 ] && echo "   $ADDED permission(s) added" || echo "   Permissions set"

EXPECTED='{"type":"command","command":"~/.claude/statusline.sh","padding":0}'
CURRENT=$(jq -c '.statusLine // {}' "$SETTINGS_JSON" 2>/dev/null)
if [ "$CURRENT" != "$EXPECTED" ]; then
    jq '.statusLine = {"type":"command","command":"~/.claude/statusline.sh","padding":0}' "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
    echo "   Statusline configured"
fi

echo ""
echo "=== Done (v${NEW_VERSION}). Restart Claude Code to activate. ==="
