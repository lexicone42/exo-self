#!/usr/bin/env bash
# env.sh — Shared environment setup for all exo-self hook handlers
#
# Claude Code hooks run with a minimal PATH that may not include
# user-installed tools. This ensures uv, python3, jq, and git are found
# on both Linux and macOS (Homebrew).
#
# Source this at the top of every hook handler:
#   source "$(dirname "$0")/env.sh"

# Homebrew paths (macOS Apple Silicon + Intel)
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

# Common Linux paths
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
