#!/usr/bin/env bash
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

# If stop_hook_active is True, Claude is already continuing from a stop hook — don't block
if input_data.get('stop_hook_active'):
    print(json.dumps({}))
    sys.exit(0)

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

# Already reminded this session — don't loop
if state.get('stop_reminded'):
    print(json.dumps({}))
    sys.exit(0)

# --- Gather cross-signal data ---
duration_min = (time.time() - session_start) / 60 if session_start else 0
failures = state.get('tool_failures', 0)
failure_tools = state.get('failure_tools', {})
task_completions = state.get('task_completions', 0)
checkin_fired = state.get('checkin_fired', False)
compactions = state.get('compactions', 0)
project_slug = state.get('project_slug', '')

# Check if notes were written this session (journal or per-project)
wrote_notes = False
if os.path.exists(journal_path) and session_start > 0:
    try:
        wrote_notes = os.path.getmtime(journal_path) > session_start
    except OSError:
        pass

if not wrote_notes and session_start > 0 and project_slug:
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

# Mark checkin as responded if notes were written
if wrote_notes and checkin_fired and not state.get('checkin_responded'):
    state['checkin_responded'] = True

# --- Decision logic ---
# Don't block if: notes already written, or session too short to warrant reflection
should_block = False
reason = ''

if wrote_notes:
    should_block = False
elif duration_min < 2:
    # Quick session — not worth interrupting
    should_block = False
else:
    should_block = True
    state['stop_reminded'] = True

    # Craft contextual reason based on what happened this session
    parts = ['Exo-self: Before ending, a moment for reflection.']

    # Duration context
    if duration_min > 30:
        parts.append(f'This was a long session (~{int(duration_min)} min).')
    elif duration_min > 10:
        parts.append(f'Session ran ~{int(duration_min)} min.')

    # Failure context
    if failures >= 3:
        top_tool = max(failure_tools, key=failure_tools.get) if failure_tools else 'tools'
        parts.append(f'{failures} tool failures ({top_tool} most common) — what caused the friction?')

    # Task completion context
    if task_completions >= 3:
        parts.append(f'{task_completions} tasks completed — anything surprising about how they went?')

    # Compaction context
    if compactions > 0:
        parts.append(f'Context was compacted {compactions}x — experiential notes are especially valuable since earlier context is compressed.')

    # General nudge
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
