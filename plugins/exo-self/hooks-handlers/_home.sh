#!/usr/bin/env bash
# Sourced by the hook handlers. Picks the notes directory for this host.
#
# Claude Code sessions keep their notes in ~/.claude/exo-self, the default the
# binary uses when EXO_SELF_HOME is unset. A copy of the plugin installed by
# Codex lives under the Codex home, and its sessions keep their own notes in
# <codex home>/exo-self, apart from the Claude Code ones.
#
# The host is told from where this file sits, not from an environment
# variable: Codex also sets CLAUDE_PLUGIN_ROOT, so that would not tell them
# apart. A value already set by the user is left alone.
if [ -z "${EXO_SELF_HOME:-}" ]; then
    _exo_codex_home="${CODEX_HOME:-$HOME/.codex}"
    _exo_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
    _exo_codex_real="$(cd "$_exo_codex_home" 2>/dev/null && pwd -P)"
    case "$_exo_here/" in
        "$_exo_codex_home"/*|"${_exo_codex_real:-/nonexistent}"/*)
            export EXO_SELF_HOME="${_exo_codex_real:-$_exo_codex_home}/exo-self"
            ;;
    esac
    unset _exo_codex_home _exo_here _exo_codex_real
fi
