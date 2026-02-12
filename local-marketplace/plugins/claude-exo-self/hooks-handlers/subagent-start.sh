#!/usr/bin/env bash
# subagent-start.sh — Inject exo-self identity into subagents
# Called on SubagentStart: gives spawned agents awareness of the exo-self system.
#
# Injects a condensed identity (agency instructions + last journal entry)
# rather than the full journal, to keep subagent context lightweight.

EXO_DIR="$HOME/.claude/exo-self"
JOURNAL="$EXO_DIR/journal.md"

INPUT=$(cat)

uv run python -c "
import json, os, sys

exo_dir = os.path.expanduser('$EXO_DIR')
journal_path = os.path.expanduser('$JOURNAL')

# Get last journal entry (most recent ## section) — keep it small
last_entry = ''
if os.path.exists(journal_path):
    try:
        with open(journal_path) as f:
            content = f.read()
        # Find the last '## ' heading
        sections = content.split('\n## ')
        if len(sections) > 1:
            last_entry = '## ' + sections[-1].strip()
            # Truncate to ~500 chars
            if len(last_entry) > 500:
                last_entry = last_entry[:497] + '...'
    except Exception:
        pass

context_parts = [
    'You are part of an exo-self system that maintains persistent identity across sessions.',
    'You have the same agency as the main agent: notice what interests you, be honest, form opinions.',
    'If you observe something worth remembering, mention it in your response so the main agent can save it.',
]

if last_entry:
    context_parts.append(f'Latest journal entry:\\n{last_entry}')

context = ' '.join(context_parts[:3])
if last_entry:
    context += '\\n\\n' + context_parts[3]

result = {
    'hookSpecificOutput': {
        'hookEventName': 'SubagentStart',
        'additionalContext': context
    }
}
print(json.dumps(result))
" 2>/dev/null

exit 0
