#!/usr/bin/env bash
# deploy.sh — Install or update claude-exo-self plugin from source
#
# Usage:
#   ./deploy.sh          # Install/update plugin
#   ./deploy.sh --check  # Show what would change without modifying anything
#
# Prerequisites: uv (https://astral.sh/uv), jq
# Works on Linux and macOS.

set -euo pipefail

PLUGIN_NAME="claude-exo-self"
MARKETPLACE="exo-self"
PLUGIN_KEY="${PLUGIN_NAME}@${MARKETPLACE}"

# Script's directory = plugin source root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Extract version from CHANGELOG.md (first ## N.N.N line)
VERSION=$(grep -m1 '^## [0-9]' "$SCRIPT_DIR/CHANGELOG.md" | sed 's/^## //')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from CHANGELOG.md"
    exit 1
fi

CLAUDE_DIR="$HOME/.claude"
CACHE_DIR="$CLAUDE_DIR/plugins/cache/local/$PLUGIN_NAME/$VERSION"
EXO_DIR="$HOME/.claude/exo-self"
INSTALLED_JSON="$CLAUDE_DIR/plugins/installed_plugins.json"
SETTINGS_JSON="$CLAUDE_DIR/settings.json"

CHECK_ONLY=false
[ "${1:-}" = "--check" ] && CHECK_ONLY=true

echo "=== claude-exo-self deploy v${VERSION} ==="
echo ""

# --- Check prerequisites ---
if ! command -v uv &>/dev/null; then
    echo "ERROR: uv is required but not found."
    echo "Install: curl -LsSf https://astral.sh/uv/install.sh | sh"
    exit 1
fi

if ! command -v jq &>/dev/null; then
    echo "ERROR: jq is required but not found (used by statusline)."
    echo "Install: brew install jq  (macOS) or apt install jq  (Linux)"
    exit 1
fi

if [ ! -d "$CLAUDE_DIR" ]; then
    echo "ERROR: ~/.claude directory not found. Is Claude Code installed?"
    exit 1
fi

if $CHECK_ONLY; then
    echo "[check] Source:  $SCRIPT_DIR"
    echo "[check] Version: $VERSION"
    echo "[check] Cache:   $CACHE_DIR"
    echo ""

    if [ -d "$CACHE_DIR" ]; then
        echo "[check] Cache directory exists. Changes:"
        diff -rq "$SCRIPT_DIR" "$CACHE_DIR" --exclude=deploy.sh 2>/dev/null || true
    else
        # Check for old versions
        OLD_DIRS=$(find "$CLAUDE_DIR/plugins/cache/local/$PLUGIN_NAME" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | grep -v "$VERSION" || true)
        if [ -n "$OLD_DIRS" ]; then
            echo "[check] Old versions found (will be removed):"
            echo "$OLD_DIRS"
        fi
        echo "[check] Cache directory does not exist — fresh install."
    fi
    exit 0
fi

# --- 1. Sync version in marketplace.json and plugin.json ---
echo "1. Syncing version to $VERSION..."

# marketplace.json at repo root (for marketplace installs)
MARKETPLACE_JSON="$SCRIPT_DIR/../../.claude-plugin/marketplace.json"
# plugin.json inside plugin dir (for local/cache installs)
PLUGIN_JSON="$SCRIPT_DIR/.claude-plugin/plugin.json"

for JSON_FILE in "$MARKETPLACE_JSON" "$PLUGIN_JSON"; do
    [ -f "$JSON_FILE" ] || continue
    BASENAME=$(basename "$(dirname "$JSON_FILE")")/$(basename "$JSON_FILE")
    uv run python -c "
import json, sys
with open('$JSON_FILE') as f:
    data = json.load(f)
changed = False
# marketplace.json has plugins array
for p in data.get('plugins', []):
    if p.get('name') == '$PLUGIN_NAME' and p.get('version') != '$VERSION':
        p['version'] = '$VERSION'
        changed = True
# plugin.json has top-level version
if 'plugins' not in data and data.get('version') != '$VERSION':
    data['version'] = '$VERSION'
    changed = True
if changed:
    with open('$JSON_FILE', 'w') as f:
        json.dump(data, f, indent=2)
        f.write('\n')
    print(f'   -> {\"$BASENAME\"} updated to $VERSION')
else:
    print(f'   -> {\"$BASENAME\"} already at $VERSION')
" 2>/dev/null
done

# --- 2. Sync plugin files to cache ---
echo "2. Syncing plugin to cache..."

# Remove old version directories
if [ -d "$CLAUDE_DIR/plugins/cache/local/$PLUGIN_NAME" ]; then
    find "$CLAUDE_DIR/plugins/cache/local/$PLUGIN_NAME" -mindepth 1 -maxdepth 1 -type d ! -name "$VERSION" -exec rm -rf {} + 2>/dev/null || true
fi

mkdir -p "$CACHE_DIR"

# Use rsync if available (preserves permissions, handles deletes), fall back to cp
if command -v rsync &>/dev/null; then
    rsync -a --delete --exclude=deploy.sh "$SCRIPT_DIR/" "$CACHE_DIR/"
else
    rm -rf "$CACHE_DIR"/*
    # Copy everything except deploy.sh
    find "$SCRIPT_DIR" -mindepth 1 -maxdepth 1 ! -name deploy.sh -exec cp -R {} "$CACHE_DIR/" \;
fi
echo "   -> $CACHE_DIR"

# --- 3. Create runtime directories ---
echo "3. Creating runtime directories..."
mkdir -p "$EXO_DIR"/{reflections,per-project,sessions}

# --- 4. Create default config if missing ---
if [ ! -f "$EXO_DIR/config.json" ]; then
    echo "4. Creating default config..."
    cat > "$EXO_DIR/config.json" << 'CFGEOF'
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
CFGEOF
else
    echo "4. Config exists, skipping."
fi

# --- 5. Update installed_plugins.json ---
echo "5. Updating installed_plugins.json..."
mkdir -p "$CLAUDE_DIR/plugins"

NOW=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")

uv run python << PYEOF
import json, os

installed_path = "$INSTALLED_JSON"
plugin_key = "$PLUGIN_KEY"
cache_dir = "$CACHE_DIR"
version = "$VERSION"
now = "$NOW"

# Load or create
data = {"version": 2, "plugins": {}}
if os.path.exists(installed_path):
    try:
        with open(installed_path) as f:
            data = json.load(f)
    except Exception:
        pass

entry = {
    "scope": "user",
    "installPath": cache_dir,
    "version": version,
    "installedAt": data.get("plugins", {}).get(plugin_key, [{}])[0].get("installedAt", now) if plugin_key in data.get("plugins", {}) else now,
    "lastUpdated": now,
}

data.setdefault("plugins", {})[plugin_key] = [entry]

with open(installed_path, "w") as f:
    json.dump(data, f, indent=2)

print(f"   -> {plugin_key} = {version}")
PYEOF

# --- 6. Install statusline ---
echo "6. Installing statusline..."
STATUSLINE_SRC="$SCRIPT_DIR/statusline.sh"
STATUSLINE_DST="$CLAUDE_DIR/statusline.sh"

if [ -f "$STATUSLINE_SRC" ]; then
    cp "$STATUSLINE_SRC" "$STATUSLINE_DST"
    chmod +x "$STATUSLINE_DST"
    echo "   -> $STATUSLINE_DST"
else
    echo "   -> statusline.sh not found in source, skipping."
fi

# --- 7. Enable plugin + statusline in settings.json ---
echo "7. Updating settings.json..."

uv run python << PYEOF
import json, os

settings_path = "$SETTINGS_JSON"
plugin_key = "$PLUGIN_KEY"
statusline_dst = "$STATUSLINE_DST"

data = {}
if os.path.exists(settings_path):
    try:
        with open(settings_path) as f:
            data = json.load(f)
    except Exception:
        pass

changed = False

# Enable plugin
enabled = data.setdefault("enabledPlugins", {})
if enabled.get(plugin_key) is not True:
    enabled[plugin_key] = True
    changed = True
    print("   -> Plugin enabled.")
else:
    print("   -> Plugin already enabled.")

# Configure statusline
statusline = data.get("statusLine", {})
expected = {
    "type": "command",
    "command": "~/.claude/statusline.sh",
    "padding": 0,
}
if statusline != expected:
    data["statusLine"] = expected
    changed = True
    print("   -> Statusline configured.")
else:
    print("   -> Statusline already configured.")

if changed:
    with open(settings_path, "w") as f:
        json.dump(data, f, indent=2)
PYEOF

# --- Done ---
echo ""
echo "=== Deployed claude-exo-self v${VERSION} ==="
echo ""
echo "Restart Claude Code to pick up the new version."
echo ""
echo "Runtime data: $EXO_DIR"
echo "Plugin cache: $CACHE_DIR"
