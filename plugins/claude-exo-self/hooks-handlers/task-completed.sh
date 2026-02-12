#!/usr/bin/env bash
# task-completed.sh — Micro-reflection prompt when tasks complete
# Called on TaskCompleted: fires when a task is marked as completed
# (via TaskUpdate tool or when a teammate finishes with in-progress tasks).
#
# Injects a brief reflection prompt — not every time, but when enough
# tasks have been completed to suggest a meaningful chunk of work is done.
#
# NOTE: This hook uses exit code 2 to optionally prevent premature
# task completion, but we don't block — we only inject context.

EXO_DIR="$HOME/.claude/exo-self"
SESSIONS_DIR="$EXO_DIR/sessions"

INPUT=$(cat)

uv run python -c "
import json, os, sys, time

input_data = json.loads('''$INPUT''') if '''$INPUT'''.strip() else {}

session_id = input_data.get('session_id', '')
if not session_id:
    print('{}')
    sys.exit(0)

exo_dir = os.path.expanduser('$EXO_DIR')
sessions_dir = os.path.expanduser('$SESSIONS_DIR')

# Load session state
state = {}
state_path = os.path.join(sessions_dir, f'state-{session_id}.json')
if os.path.exists(state_path):
    try:
        with open(state_path) as f:
            state = json.load(f)
    except Exception:
        pass

# Track task completions
completions = state.get('task_completions', 0) + 1
state['task_completions'] = completions
state['last_task_completed_at'] = time.time()

result = {}

# Nudge on every 3rd task completion (not every single one — that's noisy)
if completions % 3 == 0 and not state.get('task_reflection_suppressed'):
    msg = (
        f'## Exo-Self: Task Milestone ({completions} tasks completed)\n\n'
        f'You have completed {completions} tasks this session. '
        f'Quick gut check — how is this session going? '
        f'Anything worth a sentence in per-project notes before moving on?'
    )
    result['hookSpecificOutput'] = {
        'hookEventName': 'TaskCompleted',
        'additionalContext': msg,
    }

# Save state
try:
    os.makedirs(sessions_dir, exist_ok=True)
    with open(state_path, 'w') as f:
        json.dump(state, f)
except Exception:
    pass

print(json.dumps(result))
" 2>/dev/null

exit 0
