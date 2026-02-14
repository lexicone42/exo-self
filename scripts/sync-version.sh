#!/usr/bin/env bash
# Sync version from Cargo.toml to plugin.json (and marketplace.json if present)
# Called by pre-commit hook.

set -euo pipefail

PLUGIN_NAME="exo-self"
CARGO_TOML="plugins/$PLUGIN_NAME/Cargo.toml"
MARKETPLACE_JSON=".claude-plugin/marketplace.json"
PLUGIN_JSON="plugins/$PLUGIN_NAME/.claude-plugin/plugin.json"

# Extract version from Cargo.toml (authoritative source)
VERSION=$(grep -m1 '^version' "$CARGO_TOML" | sed 's/.*"\(.*\)".*/\1/')
if [ -z "$VERSION" ]; then
    echo "sync-version: could not extract version from CHANGELOG.md"
    exit 1
fi

CHANGED=false

# Update marketplace.json
if [ -f "$MARKETPLACE_JSON" ]; then
    CURRENT=$(jq -r ".plugins[] | select(.name == \"$PLUGIN_NAME\") | .version" "$MARKETPLACE_JSON")
    if [ "$CURRENT" != "$VERSION" ]; then
        jq "(.plugins[] | select(.name == \"$PLUGIN_NAME\") | .version) = \"$VERSION\"" \
            "$MARKETPLACE_JSON" > "$MARKETPLACE_JSON.tmp" && mv "$MARKETPLACE_JSON.tmp" "$MARKETPLACE_JSON"
        git add "$MARKETPLACE_JSON"
        echo "sync-version: marketplace.json -> $VERSION"
        CHANGED=true
    fi
fi

# Update plugin.json
if [ -f "$PLUGIN_JSON" ]; then
    CURRENT=$(jq -r '.version' "$PLUGIN_JSON")
    if [ "$CURRENT" != "$VERSION" ]; then
        jq ".version = \"$VERSION\"" "$PLUGIN_JSON" > "$PLUGIN_JSON.tmp" && mv "$PLUGIN_JSON.tmp" "$PLUGIN_JSON"
        git add "$PLUGIN_JSON"
        echo "sync-version: plugin.json -> $VERSION"
        CHANGED=true
    fi
fi

if $CHANGED; then
    echo "sync-version: synced to v$VERSION"
fi
