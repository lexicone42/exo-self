#!/usr/bin/env bash
# Platform-aware: macOS + Linux sandbox may share ~/.claude via bind mount
_DIR="$(dirname "$0")/bin"
_PLAT="$(uname -s)-$(uname -m)"
case "$_PLAT" in
    Linux-x86_64)  _BIN="$_DIR/exo-self-linux-x64" ;;
    Linux-aarch64) _BIN="$_DIR/exo-self-linux-arm64" ;;
    *)             _BIN="$_DIR/exo-self" ;;
esac
[ ! -x "$_BIN" ] && _BIN="$_DIR/exo-self"
exec "$_BIN" statusline
