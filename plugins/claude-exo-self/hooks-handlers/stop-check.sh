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

if wrote_notes:
    should_block = False
elif duration_min < 2:
    should_block = False
else:
    should_block = True
    state['stop_reminded'] = True

    parts = ['Exo-self: Before ending, a moment for reflection.']

    if duration_min > 30:
        parts.append(f'This was a long session (~{int(duration_min)} min).')
    elif duration_min > 10:
        parts.append(f'Session ran ~{int(duration_min)} min.')

    if failures >= 3:
        top_tool = max(failure_tools, key=failure_tools.get) if failure_tools else 'tools'
        parts.append(f'{failures} tool failures ({top_tool} most common) — what caused the friction?')

    if task_completions >= 3:
        parts.append(f'{task_completions} tasks completed — anything surprising about how they went?')

    if compactions > 0:
        parts.append(f'Context was compacted {compactions}x — experiential notes are especially valuable since earlier context is compressed.')

    target = f'per-project/{project_slug}.md' if project_slug else 'journal.md'
    parts.append(f'Even a sentence in ~/.claude/exo-self/{target} helps future-you. If genuinely nothing to note, acknowledge and stop.')

    reason = ' '.join(parts)

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
