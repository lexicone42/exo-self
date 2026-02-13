#!/usr/bin/env bash
source "$(dirname "$0")/env.sh"
# stop-check.sh — Context-aware nudge to write notes before session ends
# Called on Stop: evaluates whether this session warrants reflection.
#
# Uses cross-signal data (duration, failures, task completions, check-in
# state) to decide whether to block, and crafts a contextual message
# about what specifically might be worth noting.
#
# Short sessions (<2 min) or sessions where notes were already written
# pass through without blocking.
#
# Session-aware: uses session_id from stdin to find the correct
# session-specific state file (matches context-monitor.py behavior).

EXO_DIR="$HOME/.claude/exo-self"
JOURNAL="$EXO_DIR/journal.md"
META="$EXO_DIR/meta.json"
SESSIONS_DIR="$EXO_DIR/sessions"

# Read hook input from stdin
INPUT=$(cat)

uv run python -c "
import json, os, sys, datetime, time

input_data = json.loads('''$INPUT''') if '''$INPUT'''.strip() else {}

exo_dir = os.path.expanduser('$EXO_DIR')
journal_path = os.path.expanduser('$JOURNAL')
meta_path = os.path.expanduser('$META')
sessions_dir = os.path.expanduser('$SESSIONS_DIR')
shared_state_path = os.path.join(exo_dir, '.context-monitor-state.json')

# Find session-specific state file
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
checkin_fired = state.get('checkin_fired', False)
project_slug = state.get('project_slug', '')

# --- Bookkeeping BEFORE early-exit guards ---
# Detect wrote_notes via mtime
wrote_notes = False
if os.path.exists(journal_path) and session_start > 0:
    try:
        wrote_notes = os.path.getmtime(journal_path) > session_start
    except OSError:
        pass

proj_path = ''
if not wrote_notes and session_start > 0 and project_slug:
    proj_path = os.path.join(exo_dir, 'per-project', f'{project_slug}.md')
    if os.path.exists(proj_path):
        try:
            wrote_notes = os.path.getmtime(proj_path) > session_start
        except OSError:
            pass

# Content-based fallback: check for check-in markers in NEW content only
if not wrote_notes and session_start > 0 and project_slug:
    if not proj_path:
        proj_path = os.path.join(exo_dir, 'per-project', f'{project_slug}.md')
    if os.path.exists(proj_path):
        try:
            start_size = state.get('per_project_filesize', 0)
            with open(proj_path) as f:
                f.seek(start_size)
                new_content = f.read()
            if new_content and ('**Friction**' in new_content or '### Check-in' in new_content or '**Spark**' in new_content):
                wrote_notes = True
        except Exception:
            pass

# Update checkin_responded BEFORE guards — this is the key fix
if wrote_notes and checkin_fired and not state.get('checkin_responded'):
    state['checkin_responded'] = True
    try:
        with open(state_path, 'w') as f:
            json.dump(state, f)
    except Exception:
        pass

# --- Early-exit guards (prevent re-blocking, but bookkeeping is already done) ---
if input_data.get('stop_hook_active'):
    print(json.dumps({}))
    sys.exit(0)

if state.get('stop_reminded'):
    print(json.dumps({}))
    sys.exit(0)

# Cooldown: don't block again within 60s of last stop event
last_stop = state.get('last_stop_time', 0)
if last_stop and (time.time() - last_stop) < 60:
    print(json.dumps({}))
    sys.exit(0)

# --- Gather cross-signal data ---
duration_min = (time.time() - session_start) / 60 if session_start else 0
failures = state.get('tool_failures', 0)
failure_tools = state.get('failure_tools', {})
task_completions = state.get('task_completions', 0)
compactions = state.get('compactions', 0)

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

# --- Decision logic ---
should_block = False
reason = ''

# Only block if: (a) no notes written, (b) session was substantial (>5 min),
# and (c) something happened worth reflecting on (check-in fired, failures, tasks, or compactions)
has_signal = checkin_fired or failures >= 3 or task_completions >= 2 or compactions > 0

if wrote_notes:
    should_block = False
elif duration_min < 5:
    should_block = False
elif not has_signal:
    # Short-ish session with nothing notable — don't interrupt
    should_block = False
else:
    should_block = True
    state['stop_reminded'] = True
    state['last_stop_time'] = time.time()

    target = f'per-project/{project_slug}.md' if project_slug else 'journal.md'
    reason = f'Exo-self: ~{int(duration_min)} min session'
    if failures >= 3:
        top_tool = max(failure_tools, key=failure_tools.get) if failure_tools else 'tools'
        reason += f', {failures} failures ({top_tool})'
    if task_completions >= 2:
        reason += f', {task_completions} tasks done'
    if compactions > 0:
        reason += f', {compactions}x compacted'
    reason += f'. A sentence to ~/{target}? If nothing to note, just stop.'

# Persist state
try:
    with open(state_path, 'w') as f:
        json.dump(state, f)
except Exception:
    pass

if should_block:
    print(json.dumps({'decision': 'block', 'reason': reason}))
else:
    print(json.dumps({}))
" 2>/dev/null

exit 0
