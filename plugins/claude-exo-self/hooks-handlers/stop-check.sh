#!/usr/bin/env bash
# stop-check.sh — Gentle nudge to write notes before session ends
# Called on Stop: checks if Claude wrote any exo-self notes this session
#
# Uses stop_hook_active from stdin (provided by Claude Code) to avoid
# the stop-hook loop — if Claude is already continuing due to a stop hook,
# we don't block again.
#
# Session-aware: uses session_id from stdin to find the correct
# session-specific state file (matches context-monitor.py behavior).

EXO_DIR="$HOME/.claude/exo-self"
JOURNAL="$EXO_DIR/journal.md"
META="$EXO_DIR/meta.json"
SESSIONS_DIR="$EXO_DIR/sessions"

# Read hook input from stdin
INPUT=$(cat)

# Single Python invocation: parse input, check state + journal, update meta
RESULT=$(uv run python -c "
import json, os, sys, datetime, time

input_data = json.loads('''$INPUT''') if '''$INPUT'''.strip() else {}

# If stop_hook_active is True, Claude is already continuing from a stop hook — don't block
if input_data.get('stop_hook_active'):
    print('skip')
    sys.exit(0)

exo_dir = os.path.expanduser('$EXO_DIR')
journal_path = os.path.expanduser('$JOURNAL')
meta_path = os.path.expanduser('$META')
sessions_dir = os.path.expanduser('$SESSIONS_DIR')
shared_state_path = os.path.join(exo_dir, '.context-monitor-state.json')

# Find session-specific state file (same logic as context-monitor.py)
session_id = input_data.get('session_id', '')
state_path = shared_state_path
if session_id:
    candidate = os.path.join(sessions_dir, f'state-{session_id}.json')
    if os.path.exists(candidate):
        state_path = candidate

# Load state
state = {}
try:
    with open(state_path) as f:
        state = json.load(f)
except Exception:
    pass

session_start = state.get('session_start', 0)

# If we already sent the stop reminder this session, don't block again
if state.get('stop_reminded'):
    print('skip')
    sys.exit(0)

# Check if journal was modified this session
wrote_notes = False
if os.path.exists(journal_path) and session_start > 0:
    try:
        wrote_notes = os.path.getmtime(journal_path) > session_start
    except OSError:
        pass

# Also check per-project notes
if not wrote_notes and session_start > 0:
    project_slug = state.get('project_slug', '')
    if project_slug:
        proj_path = os.path.join(exo_dir, 'per-project', f'{project_slug}.md')
        if os.path.exists(proj_path):
            try:
                wrote_notes = os.path.getmtime(proj_path) > session_start
            except OSError:
                pass

# Update meta with session end time
try:
    if os.path.exists(meta_path):
        with open(meta_path) as f:
            meta = json.load(f)
        meta['last_session_end'] = datetime.datetime.now().isoformat()
        with open(meta_path, 'w') as f:
            json.dump(meta, f, indent=2)
except Exception:
    pass

# If notes were written and checkin had fired, mark checkin as responded
if wrote_notes and state.get('checkin_fired') and not state.get('checkin_responded'):
    state['checkin_responded'] = True

if not wrote_notes:
    # Mark that we've sent the reminder so we don't loop
    state['stop_reminded'] = True

# Always persist state updates (checkin_responded or stop_reminded)
try:
    with open(state_path, 'w') as f:
        json.dump(state, f)
except Exception:
    pass

print('true' if wrote_notes else 'false')
" 2>/dev/null)

if [ "$RESULT" = "skip" ] || [ "$RESULT" = "true" ]; then
    # Either already reminded, already active from stop hook, or notes were written
    echo '{}'
else
    cat << 'EOF'
{
  "decision": "block",
  "reason": "Exo-self reminder: Before ending, consider whether there's anything worth noting in your journal (~/.claude/exo-self/journal.md). Even a single sentence about what this session was like. If there's genuinely nothing to note, that's fine — just acknowledge this prompt and continue stopping."
}
EOF
fi

exit 0
