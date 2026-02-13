#!/usr/bin/env bash
# deploy.sh — Install or update claude-exo-self plugin from source
#
# Usage:
#   ./deploy.sh          # Install/update plugin
#   ./deploy.sh --check  # Show what would change without modifying anything
#
# Prerequisites: uv (https://astral.sh/uv), jq, git
# Works on Linux and macOS.
#
# IMPORTANT: This script registers the plugin as a GitHub-sourced marketplace,
# NOT as a local plugin. Claude Code has a bug (#14410) where hooks from local
# (cache/local/) plugins are loaded but never executed. GitHub-sourced plugins
# work correctly.

set -euo pipefail

PLUGIN_NAME="claude-exo-self"
MARKETPLACE="exo-self"
PLUGIN_KEY="${PLUGIN_NAME}@${MARKETPLACE}"
GITHUB_REPO="lexicone42/exo-self"

# Script's directory = plugin source root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Extract version from CHANGELOG.md (first ## N.N.N line)
VERSION=$(grep -m1 '^## [0-9]' "$SCRIPT_DIR/CHANGELOG.md" | sed 's/^## //')
if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from CHANGELOG.md"
    exit 1
fi

CLAUDE_DIR="$HOME/.claude"
CACHE_DIR="$CLAUDE_DIR/plugins/cache/$MARKETPLACE/$PLUGIN_NAME/$VERSION"
MARKETPLACE_DIR="$CLAUDE_DIR/plugins/marketplaces/$MARKETPLACE"
OLD_LOCAL_CACHE="$CLAUDE_DIR/plugins/cache/local/$PLUGIN_NAME"
KNOWN_MKT_JSON="$CLAUDE_DIR/plugins/known_marketplaces.json"
EXO_DIR="$HOME/.claude/exo-self"
INSTALLED_JSON="$CLAUDE_DIR/plugins/installed_plugins.json"
SETTINGS_JSON="$CLAUDE_DIR/settings.json"

CHECK_ONLY=false
[ "${1:-}" = "--check" ] && CHECK_ONLY=true

echo "=== claude-exo-self deploy v${VERSION} ==="
echo ""

# --- Check prerequisites ---
for cmd in uv jq git; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: $cmd is required but not found."
        case "$cmd" in
            uv)  echo "Install: curl -LsSf https://astral.sh/uv/install.sh | sh" ;;
            jq)  echo "Install: brew install jq  (macOS) or apt install jq  (Linux)" ;;
            git) echo "Install: brew install git  (macOS) or apt install git  (Linux)" ;;
        esac
        exit 1
    fi
done

if [ ! -d "$CLAUDE_DIR" ]; then
    echo "ERROR: ~/.claude directory not found. Is Claude Code installed?"
    exit 1
fi

if $CHECK_ONLY; then
    echo "[check] Source:      $SCRIPT_DIR"
    echo "[check] Version:     $VERSION"
    echo "[check] Marketplace: $MARKETPLACE_DIR"
    echo "[check] Cache:       $CACHE_DIR"
    echo ""

    if [ -d "$OLD_LOCAL_CACHE" ]; then
        echo "[check] WARNING: Old cache/local/ installation found (will be removed):"
        echo "        $OLD_LOCAL_CACHE"
        echo ""
    fi

    if [ -d "$CACHE_DIR" ]; then
        echo "[check] Cache directory exists. Changes:"
        diff -rq "$SCRIPT_DIR" "$CACHE_DIR" --exclude=deploy.sh 2>/dev/null || true
    else
        # Check for old versions in the new cache path
        OLD_DIRS=$(find "$CLAUDE_DIR/plugins/cache/$MARKETPLACE/$PLUGIN_NAME" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | grep -v "$VERSION" || true)
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

# --- 2. Register marketplace in known_marketplaces.json ---
echo "2. Registering marketplace..."
mkdir -p "$CLAUDE_DIR/plugins"

uv run python << PYEOF
import json, os

mkt_path = "$KNOWN_MKT_JSON"
marketplace = "$MARKETPLACE"
github_repo = "$GITHUB_REPO"
mkt_dir = "$MARKETPLACE_DIR"

data = {}
if os.path.exists(mkt_path):
    try:
        with open(mkt_path) as f:
            data = json.load(f)
    except Exception:
        pass

if marketplace in data:
    # Update installLocation in case it changed
    data[marketplace]["installLocation"] = mkt_dir
    print(f"   -> {marketplace} already registered, updated installLocation")
else:
    from datetime import datetime, timezone
    data[marketplace] = {
        "source": {
            "source": "github",
            "repo": github_repo
        },
        "installLocation": mkt_dir,
        "lastUpdated": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")
    }
    print(f"   -> Registered {marketplace} (github: {github_repo})")

with open(mkt_path, "w") as f:
    json.dump(data, f, indent=2)
PYEOF

# --- 3. Setup marketplace directory ---
echo "3. Setting up marketplace directory..."

if [ ! -d "$MARKETPLACE_DIR/.git" ]; then
    echo "   -> Cloning from GitHub..."
    rm -rf "$MARKETPLACE_DIR"
    git clone --quiet "https://github.com/${GITHUB_REPO}.git" "$MARKETPLACE_DIR"
    echo "   -> Cloned $GITHUB_REPO"
else
    echo "   -> Marketplace clone exists"
fi

# Overlay local plugin files onto the clone so local changes take effect
# This syncs the plugin source dir into the marketplace's expected plugin location
MARKETPLACE_PLUGIN_DIR="$MARKETPLACE_DIR/plugins/$PLUGIN_NAME"
mkdir -p "$MARKETPLACE_PLUGIN_DIR"

if command -v rsync &>/dev/null; then
    rsync -a --delete --exclude=deploy.sh "$SCRIPT_DIR/" "$MARKETPLACE_PLUGIN_DIR/"
else
    rm -rf "$MARKETPLACE_PLUGIN_DIR"/*
    find "$SCRIPT_DIR" -mindepth 1 -maxdepth 1 ! -name deploy.sh -exec cp -R {} "$MARKETPLACE_PLUGIN_DIR/" \;
fi

# Also sync marketplace.json to the clone's root .claude-plugin/
if [ -f "$MARKETPLACE_JSON" ]; then
    mkdir -p "$MARKETPLACE_DIR/.claude-plugin"
    cp "$MARKETPLACE_JSON" "$MARKETPLACE_DIR/.claude-plugin/marketplace.json"
fi
echo "   -> Synced local files to marketplace clone"

# --- 4. Sync plugin files to cache ---
echo "4. Syncing plugin to cache..."

# Remove old version directories in the new cache path
if [ -d "$CLAUDE_DIR/plugins/cache/$MARKETPLACE/$PLUGIN_NAME" ]; then
    find "$CLAUDE_DIR/plugins/cache/$MARKETPLACE/$PLUGIN_NAME" -mindepth 1 -maxdepth 1 -type d ! -name "$VERSION" -exec rm -rf {} + 2>/dev/null || true
fi

mkdir -p "$CACHE_DIR"

if command -v rsync &>/dev/null; then
    rsync -a --delete --exclude=deploy.sh "$SCRIPT_DIR/" "$CACHE_DIR/"
else
    rm -rf "$CACHE_DIR"/*
    find "$SCRIPT_DIR" -mindepth 1 -maxdepth 1 ! -name deploy.sh -exec cp -R {} "$CACHE_DIR/" \;
fi
echo "   -> $CACHE_DIR"

# --- 5. Clean up old cache/local/ installation ---
if [ -d "$OLD_LOCAL_CACHE" ]; then
    echo "5. Removing old cache/local/ installation..."
    rm -rf "$OLD_LOCAL_CACHE"
    echo "   -> Removed $OLD_LOCAL_CACHE"
    # Clean up empty parent if no other local plugins remain
    rmdir "$CLAUDE_DIR/plugins/cache/local" 2>/dev/null || true
else
    echo "5. No old cache/local/ to clean up."
fi

# --- 6. Create runtime directories ---
echo "6. Creating runtime directories..."
mkdir -p "$EXO_DIR"/{reflections,per-project,sessions}

# --- 7. Create default config if missing ---
if [ ! -f "$EXO_DIR/config.json" ]; then
    echo "7. Creating default config..."
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
    echo "7. Config exists, skipping."
fi

# --- 8. Update installed_plugins.json ---
echo "8. Updating installed_plugins.json..."

# Get git commit SHA for marketplace-compatible format
GIT_SHA=$(git -C "$SCRIPT_DIR" rev-parse HEAD 2>/dev/null || echo "")

NOW=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")

uv run python << PYEOF
import json, os

installed_path = "$INSTALLED_JSON"
plugin_key = "$PLUGIN_KEY"
cache_dir = "$CACHE_DIR"
version = "$VERSION"
now = "$NOW"
git_sha = "$GIT_SHA"

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

if git_sha:
    entry["gitCommitSha"] = git_sha

data.setdefault("plugins", {})[plugin_key] = [entry]

with open(installed_path, "w") as f:
    json.dump(data, f, indent=2)

print(f"   -> {plugin_key} = {version}" + (f" ({git_sha[:12]})" if git_sha else ""))
PYEOF

# --- 9. Install statusline ---
echo "9. Installing statusline..."
STATUSLINE_SRC="$SCRIPT_DIR/statusline.sh"
STATUSLINE_DST="$CLAUDE_DIR/statusline.sh"

if [ -f "$STATUSLINE_SRC" ]; then
    cp "$STATUSLINE_SRC" "$STATUSLINE_DST"
    chmod +x "$STATUSLINE_DST"
    echo "   -> $STATUSLINE_DST"
else
    echo "   -> statusline.sh not found in source, skipping."
fi

# --- 10. Enable plugin + statusline in settings.json ---
echo "10. Updating settings.json..."

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
home = os.path.expanduser("~")
exo_dir = f"{home}/.claude/exo-self"

# Ensure exo-self permissions are allowed
allows = data.setdefault("permissions", {}).setdefault("allow", [])
needed_allows = [
    # File access for exo-self data (journal, interests, per-project notes, etc.)
    f"Read({exo_dir}/**)",
    f"Write({exo_dir}/**)",
    f"Edit({exo_dir}/**)",
    # Auto-allow all exo-self slash commands and skills
    "Skill(claude-exo-self:context-budget)",
    "Skill(claude-exo-self:exo)",
    "Skill(claude-exo-self:interests)",
    "Skill(claude-exo-self:reflect)",
    "Skill(claude-exo-self:self-reflection)",
]
added = []
for rule in needed_allows:
    if rule not in allows:
        allows.append(rule)
        changed = True
        added.append(rule)
if added:
    print(f"   -> {len(added)} permission(s) added:")
    for rule in added:
        print(f"      {rule}")
else:
    print("   -> Exo-self permissions already configured.")

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
echo "Runtime data:  $EXO_DIR"
echo "Plugin cache:  $CACHE_DIR"
echo "Marketplace:   $MARKETPLACE_DIR"
