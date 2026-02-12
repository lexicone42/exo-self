#!/usr/bin/env bash
# pre-compact.sh — Automatically save session state before compaction
#
# Called on PreCompact. Does TWO things:
# 1. Extracts a structured handoff from the transcript (automatic, no Claude needed)
# 2. Sends a shorter systemMessage reminding Claude to save subjective notes
#
# The handoff file is saved to ~/.claude/exo-self/handoffs/<session_id>.md
# and loaded by post-compact-start.sh after compaction.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXO_DIR="$HOME/.claude/exo-self"
META="$EXO_DIR/meta.json"
SESSIONS_DIR="$EXO_DIR/sessions"
HANDOFFS_DIR="$EXO_DIR/handoffs"

mkdir -p "$HANDOFFS_DIR"

# Read hook input from stdin
INPUT=$(cat)

# Extract transcript_path and session_id from input
TRANSCRIPT_PATH=$(echo "$INPUT" | uv run python -c "import sys,json; d=json.load(sys.stdin); print(d.get('transcript_path',''))" 2>/dev/null)
SESSION_ID=$(echo "$INPUT" | uv run python -c "import sys,json; d=json.load(sys.stdin); print(d.get('session_id',''))" 2>/dev/null)

# Step 1: Automatic handoff extraction from transcript
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    HANDOFF_FILE="$HANDOFFS_DIR/${SESSION_ID:-latest}.md"
    uv run python "$SCRIPT_DIR/extract-handoff.py" "$TRANSCRIPT_PATH" > "$HANDOFF_FILE" 2>/dev/null
    # Also save as "latest" for easy access
    cp "$HANDOFF_FILE" "$HANDOFFS_DIR/latest.md" 2>/dev/null
fi

# Step 2: Update state and meta, produce systemMessage
echo "$INPUT" | uv run python -c "
import json, os, sys, time, datetime

input_data = json.load(sys.stdin)

exo_dir = os.path.expanduser('$EXO_DIR')
meta_path = os.path.expanduser('$META')
sessions_dir = os.path.expanduser('$SESSIONS_DIR')
shared_state_path = os.path.join(exo_dir, '.context-monitor-state.json')

session_id = input_data.get('session_id', '')
trigger = input_data.get('trigger', 'unknown')

# Find session-specific state
state_path = shared_state_path
if session_id:
    candidate = os.path.join(sessions_dir, f'state-{session_id}.json')
    if os.path.exists(candidate):
        state_path = candidate

# Load current state
state = {}
try:
    with open(state_path) as f:
        state = json.load(f)
except Exception:
    pass

# Record compaction event
compactions = state.get('compactions', 0)
state['compactions'] = compactions + 1
state['last_compaction'] = time.time()
state['last_compaction_trigger'] = trigger

# Save updated state
try:
    with open(state_path, 'w') as f:
        json.dump(state, f)
    if state_path != shared_state_path:
        with open(shared_state_path, 'w') as f:
            json.dump(state, f)
except Exception:
    pass

# Update meta
try:
    if os.path.exists(meta_path):
        with open(meta_path) as f:
            meta = json.load(f)
        meta['total_compactions'] = meta.get('total_compactions', 0) + 1
        meta['last_compaction'] = datetime.datetime.now().isoformat()
        with open(meta_path, 'w') as f:
            json.dump(meta, f, indent=2)
except Exception:
    pass

compaction_num = state.get('compactions', 1)

# Shorter message — the factual handoff is already saved automatically
msg = f'''## Exo-Self: Pre-Compaction (#{compaction_num}, {trigger})

Session handoff has been **automatically saved** to \`~/.claude/exo-self/handoffs/\`.

If you have subjective observations worth preserving (how the work felt, patterns you noticed, things that surprised you), write them to \`journal.md\` now. Otherwise, carry on — your next instance will have the factual context.'''

result = {
    'hookSpecificOutput': {
        'hookEventName': 'PreCompact',
        'additionalContext': msg
    }
}
print(json.dumps(result))
" 2>/dev/null

exit 0
