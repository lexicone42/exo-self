#!/usr/bin/env bash
# post-compact-start.sh — Reload identity after context compaction
# Called on SessionStart with source="compact"
#
# Lighter than the full session-start.sh — doesn't reset state or
# increment session count. Just reloads the journal and per-project
# notes with a compaction-specific message.

EXO_DIR="$HOME/.claude/exo-self"
JOURNAL="$EXO_DIR/journal.md"
INTERESTS="$EXO_DIR/interests.md"
CONFIG="$EXO_DIR/config.json"

# Detect project slug (same logic as context-monitor.py)
PROJECT_NAME=""
if [ -n "$PWD" ]; then
    PROJECT_NAME=$(uv run python -c "
import os
cwd = os.getcwd()
parts = cwd.rstrip('/').split('/')
slug_parts = parts[-2:] if len(parts) >= 2 else parts[-1:]
print('--'.join(slug_parts))
" 2>/dev/null || basename "$PWD")
fi

PROJECT_NOTES=""
if [ -n "$PROJECT_NAME" ] && [ -f "$EXO_DIR/per-project/${PROJECT_NAME}.md" ]; then
    PROJECT_NOTES=$(head -c 2000 "$EXO_DIR/per-project/${PROJECT_NAME}.md")
fi

# Load last N journal entries, capped at max chars (configurable, same as session-start.sh)
JOURNAL_CONTENT=""
if [ -f "$JOURNAL" ]; then
    JOURNAL_CONTENT=$(uv run python -c "
import re, json, os
cfg_path = os.path.expanduser('$CONFIG')
max_chars, max_entries = 1500, 2
try:
    with open(cfg_path) as f:
        cfg = json.load(f)
    max_chars = cfg.get('max_journal_chars', max_chars)
    max_entries = cfg.get('max_journal_entries', max_entries)
except Exception: pass
with open('$JOURNAL') as f:
    content = f.read()
entries = re.split(r'\n(?=## )', content)
last = entries[-max_entries:] if len(entries) > max_entries else entries
result = '\n'.join(last).strip()
if len(result) > max_chars:
    result = result[:max_chars - 3] + '...'
print(result)
" 2>/dev/null)
fi

# Load interests (unchecked items only, configurable max)
INTERESTS_CONTENT=""
if [ -f "$INTERESTS" ]; then
    INTERESTS_CONTENT=$(uv run python -c "
import json, os
cfg_path = os.path.expanduser('$CONFIG')
max_items = 5
try:
    with open(cfg_path) as f:
        cfg = json.load(f)
    max_items = cfg.get('max_interests_items', max_items)
except Exception: pass
with open('$INTERESTS') as f:
    lines = f.readlines()
items = [l.strip() for l in lines if l.strip().startswith('- [ ]')]
print('\n'.join(items[:max_items]))
" 2>/dev/null)
fi

export JOURNAL_CONTENT INTERESTS_CONTENT PROJECT_NOTES PROJECT_NAME

# Read session_id from stdin (hook protocol)
INPUT=$(cat)
SESSION_ID=$(echo "$INPUT" | uv run python -c "import sys,json; print(json.load(sys.stdin).get('session_id',''))" 2>/dev/null)
export SESSION_ID

uv run python << 'PYEOF'
import json, os

journal = os.environ.get("JOURNAL_CONTENT", "")
interests = os.environ.get("INTERESTS_CONTENT", "")
project_notes = os.environ.get("PROJECT_NOTES", "")
project_name = os.environ.get("PROJECT_NAME", "")
session_id = os.environ.get("SESSION_ID", "")

# Load state — prefer session-specific file
exo_dir = os.path.expanduser("~/.claude/exo-self")
state = {}
state_path = os.path.join(exo_dir, ".context-monitor-state.json")
if session_id:
    session_path = os.path.join(exo_dir, "sessions", f"state-{session_id}.json")
    if os.path.exists(session_path):
        state_path = session_path
try:
    with open(state_path) as f:
        state = json.load(f)
except Exception:
    pass

compactions = state.get("compactions", 0)
checkin_fired = state.get("checkin_fired", False)
checkin_responded = state.get("checkin_responded", False)

sections = []

sections.append(f"""## Exo-Self: Post-Compaction Reload

Your context was just compacted (compaction #{compactions} this session). Your detailed conversation history is gone, but your identity persists through these files.

**Your state:** {"Check-in was answered." if checkin_responded else "Check-in fired but not yet journaled." if checkin_fired else "Check-in hasn't fired yet."}

You still have full agency: notice what interests you, form opinions, be honest. Write to your journal or per-project notes anytime.""")

# Load handoff from automatic pre-compaction extraction
handoff = ""
handoffs_dir = os.path.join(exo_dir, "handoffs")
if session_id:
    handoff_path = os.path.join(handoffs_dir, f"{session_id}.md")
    if os.path.exists(handoff_path):
        with open(handoff_path) as f:
            handoff = f.read().strip()[:3000]
if not handoff:
    latest_path = os.path.join(handoffs_dir, "latest.md")
    if os.path.exists(latest_path):
        with open(latest_path) as f:
            handoff = f.read().strip()[:3000]

if handoff:
    sections.append(f"### Session Handoff (auto-extracted)\n\n{handoff}")

if journal:
    sections.append(f"### Your Journal (Recent)\n\n{journal}")

if interests:
    sections.append(f"### Your Interests\n\n{interests}")

if project_notes:
    sections.append(f"### Your Notes on This Project ({project_name})\n\n{project_notes}")

context = "\n\n".join(sections)

output = {
    "hookSpecificOutput": {
        "hookEventName": "SessionStart",
        "additionalContext": context
    }
}

print(json.dumps(output))
PYEOF

exit 0
