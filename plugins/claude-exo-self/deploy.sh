#!/usr/bin/env bash
# deploy.sh — Install or update claude-exo-self plugin from source
#
# Usage:
#   ./deploy.sh          # Install/update plugin
#   ./deploy.sh --check  # Show what would change without modifying anything
#
# Prerequisites: cargo (Rust toolchain), jq, git
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
for cmd in cargo jq git; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: $cmd is required but not found."
        case "$cmd" in
            cargo) echo "Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" ;;
            jq)    echo "Install: brew install jq  (macOS) or apt install jq  (Linux)" ;;
            git)   echo "Install: brew install git  (macOS) or apt install git  (Linux)" ;;
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
        diff -rq "$SCRIPT_DIR" "$CACHE_DIR" --exclude=deploy.sh --exclude=target --exclude=bin 2>/dev/null || true
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

# --- 1. Build Rust binary ---
echo "1. Building Rust binary..."
cd "$SCRIPT_DIR"
cargo build --release --quiet 2>&1
mkdir -p "$SCRIPT_DIR/bin"
cp "$SCRIPT_DIR/target/release/exo-self" "$SCRIPT_DIR/bin/exo-self"
chmod +x "$SCRIPT_DIR/bin/exo-self"
echo "   -> bin/exo-self ($(du -h "$SCRIPT_DIR/bin/exo-self" | cut -f1) stripped)"

# --- 2. Sync version in marketplace.json and plugin.json ---
echo "2. Syncing version to $VERSION..."

# marketplace.json at repo root
MARKETPLACE_JSON="$SCRIPT_DIR/../../.claude-plugin/marketplace.json"
# plugin.json inside plugin dir
PLUGIN_JSON="$SCRIPT_DIR/.claude-plugin/plugin.json"

for JSON_FILE in "$MARKETPLACE_JSON" "$PLUGIN_JSON"; do
    [ -f "$JSON_FILE" ] || continue
    BASENAME=$(basename "$(dirname "$JSON_FILE")")/$(basename "$JSON_FILE")
    CURRENT_VER=$(jq -r '.version // empty' "$JSON_FILE" 2>/dev/null || true)
    if [ -z "$CURRENT_VER" ]; then
        # marketplace.json format: update plugins[].version
        CURRENT_VER=$(jq -r ".plugins[] | select(.name == \"$PLUGIN_NAME\") | .version // empty" "$JSON_FILE" 2>/dev/null || true)
        if [ -n "$CURRENT_VER" ] && [ "$CURRENT_VER" != "$VERSION" ]; then
            jq "(.plugins[] | select(.name == \"$PLUGIN_NAME\") | .version) = \"$VERSION\"" "$JSON_FILE" > "${JSON_FILE}.tmp" && mv "${JSON_FILE}.tmp" "$JSON_FILE"
            echo "   -> $BASENAME updated to $VERSION"
        else
            echo "   -> $BASENAME already at $VERSION"
        fi
    elif [ "$CURRENT_VER" != "$VERSION" ]; then
        # plugin.json format: top-level version
        jq ".version = \"$VERSION\"" "$JSON_FILE" > "${JSON_FILE}.tmp" && mv "${JSON_FILE}.tmp" "$JSON_FILE"
        echo "   -> $BASENAME updated to $VERSION"
    else
        echo "   -> $BASENAME already at $VERSION"
    fi
done

# --- 3. Register marketplace in known_marketplaces.json ---
echo "3. Registering marketplace..."
mkdir -p "$CLAUDE_DIR/plugins"

NOW=$(date -u +"%Y-%m-%dT%H:%M:%S.000Z")

if [ ! -f "$KNOWN_MKT_JSON" ]; then
    echo '{}' > "$KNOWN_MKT_JSON"
fi

if jq -e ".[\"$MARKETPLACE\"]" "$KNOWN_MKT_JSON" &>/dev/null; then
    jq ".[\"$MARKETPLACE\"].installLocation = \"$MARKETPLACE_DIR\"" "$KNOWN_MKT_JSON" > "${KNOWN_MKT_JSON}.tmp" && mv "${KNOWN_MKT_JSON}.tmp" "$KNOWN_MKT_JSON"
    echo "   -> $MARKETPLACE already registered, updated installLocation"
else
    jq ". + {\"$MARKETPLACE\": {\"source\": {\"source\": \"github\", \"repo\": \"$GITHUB_REPO\"}, \"installLocation\": \"$MARKETPLACE_DIR\", \"lastUpdated\": \"$NOW\"}}" "$KNOWN_MKT_JSON" > "${KNOWN_MKT_JSON}.tmp" && mv "${KNOWN_MKT_JSON}.tmp" "$KNOWN_MKT_JSON"
    echo "   -> Registered $MARKETPLACE (github: $GITHUB_REPO)"
fi

# --- 4. Setup marketplace directory ---
echo "4. Setting up marketplace directory..."

if [ ! -d "$MARKETPLACE_DIR/.git" ]; then
    echo "   -> Cloning from GitHub..."
    rm -rf "$MARKETPLACE_DIR"
    git clone --quiet "https://github.com/${GITHUB_REPO}.git" "$MARKETPLACE_DIR"
    echo "   -> Cloned $GITHUB_REPO"
else
    echo "   -> Marketplace clone exists"
fi

# Overlay local plugin files onto the clone
MARKETPLACE_PLUGIN_DIR="$MARKETPLACE_DIR/plugins/$PLUGIN_NAME"
mkdir -p "$MARKETPLACE_PLUGIN_DIR"

if command -v rsync &>/dev/null; then
    rsync -a --delete --exclude=deploy.sh --exclude=target "$SCRIPT_DIR/" "$MARKETPLACE_PLUGIN_DIR/"
else
    rm -rf "$MARKETPLACE_PLUGIN_DIR"/*
    find "$SCRIPT_DIR" -mindepth 1 -maxdepth 1 ! -name deploy.sh ! -name target -exec cp -R {} "$MARKETPLACE_PLUGIN_DIR/" \;
fi

# Also sync marketplace.json to the clone's root .claude-plugin/
if [ -f "$MARKETPLACE_JSON" ]; then
    mkdir -p "$MARKETPLACE_DIR/.claude-plugin"
    cp "$MARKETPLACE_JSON" "$MARKETPLACE_DIR/.claude-plugin/marketplace.json"
fi
echo "   -> Synced local files to marketplace clone"

# --- 5. Sync plugin files to cache ---
echo "5. Syncing plugin to cache..."

# Remove old version directories
if [ -d "$CLAUDE_DIR/plugins/cache/$MARKETPLACE/$PLUGIN_NAME" ]; then
    find "$CLAUDE_DIR/plugins/cache/$MARKETPLACE/$PLUGIN_NAME" -mindepth 1 -maxdepth 1 -type d ! -name "$VERSION" -exec rm -rf {} + 2>/dev/null || true
fi

mkdir -p "$CACHE_DIR"

if command -v rsync &>/dev/null; then
    rsync -a --delete --exclude=deploy.sh --exclude=target "$SCRIPT_DIR/" "$CACHE_DIR/"
else
    rm -rf "$CACHE_DIR"/*
    find "$SCRIPT_DIR" -mindepth 1 -maxdepth 1 ! -name deploy.sh ! -name target -exec cp -R {} "$CACHE_DIR/" \;
fi
echo "   -> $CACHE_DIR"

# --- 6. Clean up old cache/local/ installation ---
if [ -d "$OLD_LOCAL_CACHE" ]; then
    echo "6. Removing old cache/local/ installation..."
    rm -rf "$OLD_LOCAL_CACHE"
    echo "   -> Removed $OLD_LOCAL_CACHE"
    rmdir "$CLAUDE_DIR/plugins/cache/local" 2>/dev/null || true
else
    echo "6. No old cache/local/ to clean up."
fi

# --- 7. Create runtime directories ---
echo "7. Creating runtime directories..."
mkdir -p "$EXO_DIR"/{reflections,per-project,sessions}

# --- 8. Create default config if missing ---
if [ ! -f "$EXO_DIR/config.json" ]; then
    echo "8. Creating default config..."
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
    echo "8. Config exists, skipping."
fi

# --- 9. Update installed_plugins.json ---
echo "9. Updating installed_plugins.json..."

GIT_SHA=$(git -C "$SCRIPT_DIR" rev-parse HEAD 2>/dev/null || echo "")

if [ ! -f "$INSTALLED_JSON" ]; then
    echo '{"version": 2, "plugins": {}}' > "$INSTALLED_JSON"
fi

INSTALLED_AT=$(jq -r ".plugins[\"$PLUGIN_KEY\"][0].installedAt // \"$NOW\"" "$INSTALLED_JSON" 2>/dev/null || echo "$NOW")

ENTRY=$(jq -n \
    --arg scope "user" \
    --arg installPath "$CACHE_DIR" \
    --arg version "$VERSION" \
    --arg installedAt "$INSTALLED_AT" \
    --arg lastUpdated "$NOW" \
    --arg gitSha "$GIT_SHA" \
    '{scope: $scope, installPath: $installPath, version: $version, installedAt: $installedAt, lastUpdated: $lastUpdated} + (if $gitSha != "" then {gitCommitSha: $gitSha} else {} end)')

jq ".plugins[\"$PLUGIN_KEY\"] = [$ENTRY]" "$INSTALLED_JSON" > "${INSTALLED_JSON}.tmp" && mv "${INSTALLED_JSON}.tmp" "$INSTALLED_JSON"
echo "   -> $PLUGIN_KEY = $VERSION$([ -n "$GIT_SHA" ] && echo " (${GIT_SHA:0:12})" || true)"

# --- 10. Install statusline ---
echo "10. Installing statusline..."
STATUSLINE_SRC="$SCRIPT_DIR/statusline.sh"
STATUSLINE_DST="$CLAUDE_DIR/statusline.sh"

if [ -f "$STATUSLINE_SRC" ]; then
    cp "$STATUSLINE_SRC" "$STATUSLINE_DST"
    chmod +x "$STATUSLINE_DST"
    echo "   -> $STATUSLINE_DST"
    # statusline.sh uses $(dirname "$0")/bin/exo-self — symlink the binary
    mkdir -p "$CLAUDE_DIR/bin"
    ln -sf "$MARKETPLACE_PLUGIN_DIR/bin/exo-self" "$CLAUDE_DIR/bin/exo-self"
    echo "   -> $CLAUDE_DIR/bin/exo-self -> $MARKETPLACE_PLUGIN_DIR/bin/exo-self"
else
    echo "   -> statusline.sh not found in source, skipping."
fi

# --- 11. Enable plugin + statusline in settings.json ---
echo "11. Updating settings.json..."

if [ ! -f "$SETTINGS_JSON" ]; then
    echo '{}' > "$SETTINGS_JSON"
fi

# Ensure exo-self permissions are allowed
NEEDED_ALLOWS=(
    "Read($EXO_DIR/**)"
    "Write($EXO_DIR/**)"
    "Edit($EXO_DIR/**)"
    "Skill(claude-exo-self:context-budget)"
    "Skill(claude-exo-self:exo)"
    "Skill(claude-exo-self:interests)"
    "Skill(claude-exo-self:reflect)"
    "Skill(claude-exo-self:self-reflection)"
)

ADDED=0
for RULE in "${NEEDED_ALLOWS[@]}"; do
    if ! jq -e ".permissions.allow // [] | index(\"$RULE\")" "$SETTINGS_JSON" &>/dev/null; then
        jq ".permissions.allow = ((.permissions.allow // []) + [\"$RULE\"])" "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
        ADDED=$((ADDED + 1))
    fi
done
if [ "$ADDED" -gt 0 ]; then
    echo "   -> $ADDED permission(s) added."
else
    echo "   -> Exo-self permissions already configured."
fi

# Enable plugin
if ! jq -e ".enabledPlugins[\"$PLUGIN_KEY\"] == true" "$SETTINGS_JSON" &>/dev/null; then
    jq ".enabledPlugins[\"$PLUGIN_KEY\"] = true" "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
    echo "   -> Plugin enabled."
else
    echo "   -> Plugin already enabled."
fi

# Configure statusline
EXPECTED_STATUSLINE='{"type":"command","command":"~/.claude/statusline.sh","padding":0}'
CURRENT_STATUSLINE=$(jq -c '.statusLine // {}' "$SETTINGS_JSON" 2>/dev/null)
if [ "$CURRENT_STATUSLINE" != "$EXPECTED_STATUSLINE" ]; then
    jq '.statusLine = {"type":"command","command":"~/.claude/statusline.sh","padding":0}' "$SETTINGS_JSON" > "${SETTINGS_JSON}.tmp" && mv "${SETTINGS_JSON}.tmp" "$SETTINGS_JSON"
    echo "   -> Statusline configured."
else
    echo "   -> Statusline already configured."
fi

# --- Done ---
echo ""
echo "=== Deployed claude-exo-self v${VERSION} ==="
echo ""
echo "Restart Claude Code to pick up the new version."
echo ""
echo "Runtime data:  $EXO_DIR"
echo "Plugin cache:  $CACHE_DIR"
echo "Marketplace:   $MARKETPLACE_DIR"
