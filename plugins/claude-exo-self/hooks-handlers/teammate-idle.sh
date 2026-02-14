#!/usr/bin/env bash
source "$(dirname "$0")/env.sh"
# teammate-idle.sh — Inject exo-self identity into Agent Teams members
# Called on TeammateIdle: fires when a teammate finishes its current work
# and is ready for a new task assignment.
#
# Injects condensed exo-self context so team members share experiential
# awareness (not just factual knowledge from auto-memory). This is the
# Agent Teams equivalent of subagent-start.sh.
#
# NOTE: Agent Teams is a research preview feature (2026-02).
# Env: CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1

EXO_DIR="$HOME/.claude/exo-self"
JOURNAL="$EXO_DIR/journal.md"

INPUT=$(cat)

echo "$INPUT" | uv run python -c "
import json, os, sys

exo_dir = os.path.expanduser('$EXO_DIR')
journal_path = os.path.expanduser('$JOURNAL')

# Get last journal entry — keep it small for teammate context
last_entry = ''
if os.path.exists(journal_path):
    try:
        with open(journal_path) as f:
            content = f.read()
        sections = content.split('\n## ')
        if len(sections) > 1:
            last_entry = '## ' + sections[-1].strip()
            if len(last_entry) > 500:
                last_entry = last_entry[:497] + '...'
    except Exception:
        pass

# Load most recent per-project notes for this project
_raw = sys.stdin.read().strip()
input_data = json.loads(_raw) if _raw else {}
cwd = input_data.get('cwd', '')
project_notes_snippet = ''
if cwd:
    import glob
    parts = cwd.rstrip('/').split('/')
    slug_parts = parts[-2:] if len(parts) >= 2 else parts[-1:]
    slug = '--'.join(slug_parts)
    notes_dir = os.path.join(exo_dir, 'per-project', slug)
    if os.path.isdir(notes_dir):
        try:
            files = sorted(glob.glob(os.path.join(notes_dir, '*.md')), key=os.path.getmtime, reverse=True)
            if files:
                with open(files[0]) as f:
                    project_notes_snippet = f.read(800).strip()
        except Exception:
            pass

context_parts = [
    'You are part of an Agent Team with exo-self — persistent experiential identity across sessions.',
    'You share agency with the team lead: notice what interests you, be honest, form opinions.',
    'If you observe patterns worth remembering (frustrations, insights, what worked), mention them so the lead can save them to exo-self files.',
]

if project_notes_snippet:
    context_parts.append(f'Project observations so far:\n{project_notes_snippet}')
elif last_entry:
    context_parts.append(f'Latest journal entry:\n{last_entry}')

context = ' '.join(context_parts[:3])
if len(context_parts) > 3:
    context += '\n\n' + context_parts[3]

result = {
    'hookSpecificOutput': {
        'hookEventName': 'TeammateIdle',
        'additionalContext': context
    }
}
print(json.dumps(result))
" 2>/dev/null

exit 0
