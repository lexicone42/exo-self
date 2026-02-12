#!/usr/bin/env bash
# session-end.sh — Reliable session cleanup
# Called on SessionEnd: fires on ALL exits (terminal close, /clear, logout, etc.)
# More reliable than Stop hook for bookkeeping since Stop only fires on normal stops.
#
# Records session end time, duration, and exit reason in meta.json.
# Cleans up stale state.

EXO_DIR="$HOME/.claude/exo-self"
META="$EXO_DIR/meta.json"
SESSIONS_DIR="$EXO_DIR/sessions"

INPUT=$(cat)

uv run python -c "
import json, os, sys, time, datetime

input_data = json.loads('''$INPUT''') if '''$INPUT'''.strip() else {}

exo_dir = os.path.expanduser('$EXO_DIR')
meta_path = os.path.expanduser('$META')
sessions_dir = os.path.expanduser('$SESSIONS_DIR')
shared_state_path = os.path.join(exo_dir, '.context-monitor-state.json')

session_id = input_data.get('session_id', '')
reason = input_data.get('reason', 'unknown')

# Find session state to get duration
state_path = shared_state_path
if session_id:
    candidate = os.path.join(sessions_dir, f'state-{session_id}.json')
    if os.path.exists(candidate):
        state_path = candidate

state = {}
try:
    with open(state_path) as f:
        state = json.load(f)
except Exception:
    pass

session_start = state.get('session_start', 0)
duration_min = round((time.time() - session_start) / 60) if session_start else 0

# Update meta
try:
    meta = {}
    if os.path.exists(meta_path):
        with open(meta_path) as f:
            meta = json.load(f)

    meta['last_session_end'] = datetime.datetime.now().isoformat()
    meta['last_session_reason'] = reason
    meta['last_session_duration_min'] = duration_min

    # Track session history (keep last 10)
    history = meta.get('session_history', [])
    history.append({
        'session_id': session_id or state.get('session_id', ''),
        'ended': datetime.datetime.now().isoformat(),
        'reason': reason,
        'duration_min': duration_min,
        'checkin_fired': state.get('checkin_fired', False),
        'checkin_responded': state.get('checkin_responded', False),
        'compactions': state.get('compactions', 0),
    })
    meta['session_history'] = history[-10:]

    with open(meta_path, 'w') as f:
        json.dump(meta, f, indent=2)
except Exception:
    pass
" 2>/dev/null

# SessionEnd can't block or return decisions — just exit clean
echo '{}'
exit 0
