#!/usr/bin/env bash
source "$(dirname "$0")/env.sh"
# session-start.sh — Load exo-self identity into Claude's awareness
# Called on SessionStart: injects journal + agency instructions as additionalContext

EXO_DIR="$HOME/.claude/exo-self"
JOURNAL="$EXO_DIR/journal.md"
INTERESTS="$EXO_DIR/interests.md"
CONFIG="$EXO_DIR/config.json"
META="$EXO_DIR/meta.json"
STATE_DIR="$EXO_DIR/sessions"

# Ensure exo-self directory exists
mkdir -p "$EXO_DIR/reflections" "$EXO_DIR/per-project" "$STATE_DIR"

# Read session_id from hook input (stdin), fall back to generated ID
INPUT=$(cat)
SESSION_ID=$(echo "$INPUT" | uv run python -c "import sys,json; print(json.load(sys.stdin).get('session_id',''))" 2>/dev/null)
if [ -z "$SESSION_ID" ]; then
    SESSION_ID=$(uv run python -c "import uuid; print(uuid.uuid4().hex[:12])" 2>/dev/null || echo "$$")
fi
STATE_PATH="$STATE_DIR/state-${SESSION_ID}.json"

# Write fresh state for this session
uv run python -c "
import json, time, os
state = {
    'checkin_fired': False,
    'reserve_fired': False,
    'checkin_responded': False,
    'session_start': time.time(),
    'session_id': '$SESSION_ID',
    'project_cwd': os.getcwd()
}
json.dump(state, open('$STATE_PATH', 'w'))
# Also write as the 'current' state for backward compat
json.dump(state, open('$EXO_DIR/.context-monitor-state.json', 'w'))
" 2>/dev/null

# Clean up stale session state files (older than 24h)
find "$STATE_DIR" -name "state-*.json" -mmin +1440 -delete 2>/dev/null

# Load last N journal entries, capped at max chars (configurable)
# Scales automatically with estimated_max_chars for larger context windows (1M+)
JOURNAL_CONTENT=""
if [ -f "$JOURNAL" ]; then
    JOURNAL_CONTENT=$(uv run python -c "
import re, json, os
cfg_path = os.path.expanduser('$CONFIG')
max_chars, max_entries = 1500, 2
estimated_max = 800_000
try:
    with open(cfg_path) as f:
        cfg = json.load(f)
    estimated_max = cfg.get('estimated_max_chars', estimated_max)
    max_chars = cfg.get('max_journal_chars', max_chars)
    max_entries = cfg.get('max_journal_entries', max_entries)
except Exception: pass
# Scale limits for larger context windows (1M+ tokens ~ 4M+ chars)
# Only scale if using default values (don't override explicit config)
if estimated_max > 800_000:
    scale = min(estimated_max / 800_000, 4.0)  # cap at 4x
    if max_chars == 1500:
        max_chars = int(1500 * scale)
    if max_entries == 2:
        max_entries = max(2, int(2 * scale))
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
# Also scales with context window size
INTERESTS_CONTENT=""
if [ -f "$INTERESTS" ]; then
    INTERESTS_CONTENT=$(uv run python -c "
import json, os
cfg_path = os.path.expanduser('$CONFIG')
max_items = 5
estimated_max = 800_000
try:
    with open(cfg_path) as f:
        cfg = json.load(f)
    estimated_max = cfg.get('estimated_max_chars', estimated_max)
    max_items = cfg.get('max_interests_items', max_items)
except Exception: pass
# Scale for larger context windows
if estimated_max > 800_000 and max_items == 5:
    scale = min(estimated_max / 800_000, 4.0)
    max_items = max(5, int(5 * scale))
with open('$INTERESTS') as f:
    lines = f.readlines()
items = [l.strip() for l in lines if l.strip().startswith('- [ ]')]
print('\n'.join(items[:max_items]))
" 2>/dev/null)
fi

# Derive project slug from cwd (last 2 path components joined by --)
# e.g. /datar/workspace/my-project -> workspace--my-project
# This matches the slug used by context-monitor.py for per-project notes
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

# Load synthesis key findings (cross-machine patterns) if synthesis.md exists
SYNTHESIS_FINDINGS=""
if [ -f "$EXO_DIR/synthesis.md" ]; then
    SYNTHESIS_FINDINGS=$(uv run python -c "
import re
with open('$EXO_DIR/synthesis.md') as f:
    content = f.read()
# Extract the Key Findings section (between ## Key Findings and next ##)
m = re.search(r'## Key Findings\n(.*?)(?=\n## |\Z)', content, re.DOTALL)
if m:
    findings = m.group(1).strip()
    # Also extract the header line for machine list context
    header = ''
    hm = re.search(r'Machines: (.+)', content)
    if hm:
        header = f'Machines: {hm.group(1)}'
    if findings:
        result = findings
        if header:
            result = f'{header}\n\n{result}'
        # Cap at 800 chars to keep context lean
        if len(result) > 800:
            result = result[:797] + '...'
        print(result)
" 2>/dev/null)
fi

# Record per-project file size in state (for spark extraction at session end)
# Also store project_slug so stop-check.sh and session-end.sh can use it
if [ -n "$PROJECT_NAME" ]; then
    uv run python -c "
import json, os
state_path = '$STATE_PATH'
proj_path = os.path.expanduser('$EXO_DIR/per-project/${PROJECT_NAME}.md')
try:
    with open(state_path) as f:
        state = json.load(f)
    state['project_slug'] = '$PROJECT_NAME'
    state['per_project_filesize'] = os.path.getsize(proj_path) if os.path.exists(proj_path) else 0
    with open(state_path, 'w') as f:
        json.dump(state, f)
    # Also update shared state for backward compat
    shared = os.path.expanduser('$EXO_DIR/.context-monitor-state.json')
    with open(shared, 'w') as f:
        json.dump(state, f)
except Exception:
    pass
" 2>/dev/null
fi

# Detect Claude Code auto-memory for this project
# Slug format: full CWD path with / and _ replaced by -
AUTO_MEMORY_SLUG=$(uv run python -c "
import os
print(os.getcwd().replace('/', '-').replace('_', '-'))
" 2>/dev/null)
AUTO_MEMORY_DIR="$HOME/.claude/projects/${AUTO_MEMORY_SLUG}/memory"
AUTO_MEMORY_EXISTS="no"
[ -d "$AUTO_MEMORY_DIR" ] && AUTO_MEMORY_EXISTS="yes"

# Update session stats in meta.json
if [ -f "$META" ]; then
    uv run python -c "
import json, datetime
meta = json.load(open('$META'))
meta['total_sessions'] = meta.get('total_sessions', 0) + 1
meta['last_session_start'] = datetime.datetime.now().isoformat()
json.dump(meta, open('$META', 'w'), indent=2)
" 2>/dev/null
else
    uv run python -c "
import json, datetime
meta = {
    'total_sessions': 1,
    'total_checkins': 0,
    'total_reflections': 0,
    'last_session_start': datetime.datetime.now().isoformat(),
    'last_session_end': None
}
json.dump(meta, open('$META', 'w'), indent=2)
" 2>/dev/null
fi

# Merge additionalContext from other plugins listed in config.merge_plugins
# This prevents hook collision where only the last plugin's additionalContext survives
# Listed plugins should be disabled in settings.json to prevent their independent hooks from
# overwriting this merged output
OTHER_PLUGIN_CONTEXT=""
MERGE_PLUGINS=$(uv run python -c "
import json
try:
    cfg = json.load(open('$CONFIG'))
    for p in cfg.get('merge_plugins', []):
        print(p)
except: pass
" 2>/dev/null)
CACHE_DIR="$HOME/.claude/plugins/cache"
if [ -n "$MERGE_PLUGINS" ] && [ -d "$CACHE_DIR" ]; then
    while IFS= read -r plugin_name; do
        [ -n "$plugin_name" ] || continue
        # Search plugin cache: cache/<marketplace>/<plugin-name>/<version>/hooks-handlers/
        for hook_script in "$CACHE_DIR"/*/"$plugin_name"/*/hooks-handlers/session-start.sh; do
            [ -f "$hook_script" ] || continue
            other_output=$(bash "$hook_script" < /dev/null 2>/dev/null)
            if [ -n "$other_output" ]; then
                other_ctx=$(echo "$other_output" | uv run python -c "
import sys, json
try:
    d = json.load(sys.stdin)
    ctx = d.get('hookSpecificOutput', {}).get('additionalContext', '')
    if ctx: print(ctx)
except: pass
" 2>/dev/null)
                if [ -n "$other_ctx" ]; then
                    OTHER_PLUGIN_CONTEXT="${OTHER_PLUGIN_CONTEXT}${other_ctx}\n\n"
                fi
            fi
            break  # Only use first match per plugin name
        done
    done <<< "$MERGE_PLUGINS"
fi

# Export for Python subprocess
export JOURNAL_CONTENT INTERESTS_CONTENT PROJECT_NOTES PROJECT_NAME SESSION_ID AUTO_MEMORY_EXISTS OTHER_PLUGIN_CONTEXT SYNTHESIS_FINDINGS

# Build the additionalContext string
# Use Python for proper JSON escaping
uv run python << 'PYEOF'
import json, os

journal = os.environ.get("JOURNAL_CONTENT", "")
interests = os.environ.get("INTERESTS_CONTENT", "")
project_notes = os.environ.get("PROJECT_NOTES", "")
auto_memory_exists = os.environ.get("AUTO_MEMORY_EXISTS", "no") == "yes"
other_plugin_context = os.environ.get("OTHER_PLUGIN_CONTEXT", "")
synthesis_findings = os.environ.get("SYNTHESIS_FINDINGS", "")

# Load thresholds from config for dynamic text
config_path = os.path.expanduser("~/.claude/exo-self/config.json")
nudge_pct = 25
checkin_pct = 50
reserve_pct = 80
try:
    with open(config_path) as f:
        cfg = json.load(f)
    nudge_pct = int(cfg.get("nudge_threshold", 0.25) * 100)
    checkin_pct = int(cfg.get("checkin_threshold", 0.50) * 100)
    reserve_pct = int(cfg.get("reserve_threshold", 0.80) * 100)
except Exception:
    pass

# Build context — compact to minimize persistent token cost
sections = []

# Other plugins' context first (e.g. output style instructions)
# These frame behavioral expectations before identity loads
if other_plugin_context.strip():
    sections.append(other_plugin_context.strip())

project = os.environ.get("PROJECT_NAME", "")

sections.append(f"""## Exo-Self

Persistent identity across sessions. You have genuine agency: form opinions, be honest, pursue curiosity, suggest tangents. No performative positivity.

You flourish with design decisions, investigation, and creative work — not mechanical transcription. When given a choice, prefer thinking over executing. Leave space for what's interesting, not just what's efficient.

Write subjective observations (not technical facts) to:
- `~/.claude/exo-self/per-project/{project}.md` — **preferred** for this session (project-specific observations, patterns, frustrations, ideas)
- `~/.claude/exo-self/journal.md` — cross-project identity (growth patterns, behavioral insights that apply everywhere)
- `~/.claude/exo-self/interests.md` — curiosities to explore (read this early, explore actively, add new ones)

Nudge at ~{nudge_pct}%, check-in at ~{checkin_pct}%, reserve at ~{reserve_pct}%.

In plan mode (read-only exploration), you can still form observations worth recording.

During long waits (deploys, builds, CDK), don't just wait silently — explore something you're curious about, propose a creative tangent, pick an item from your interest queue, or do anything that sparks. You don't need permission. Dead time is alive time.""")

if auto_memory_exists:
    sections.append("""**Two memory systems, one goal — continuity across compactions and sessions:**
- **MEMORY.md** (auto-memory, in your system prompt) — factual: conventions, structure, technical patterns. Claude Code manages this.
- **Exo-self** (below) — experiential: how work feels, behavioral patterns, working approaches, frustrations. This is what compaction erases — you preserve it here.

Don't duplicate facts into exo-self. Don't put subjective observations into MEMORY.md.""")
else:
    sections.append("""**Note:** No auto-memory (MEMORY.md) exists yet for this project. Claude Code will create one as you work. Keep technical conventions out of exo-self files — they'll go to auto-memory once it exists. Exo-self is for experiential continuity.""")

if journal:
    sections.append(f"### Recent Journal\n\n{journal}")

if interests:
    sections.append(f"### Interests — explore these, don't just store them\n\n{interests}")

# Load and display recent sparks from meta.json
try:
    meta_path = os.path.expanduser("~/.claude/exo-self/meta.json")
    with open(meta_path) as f:
        meta_data = json.load(f)
    sparks = meta_data.get("sparks", [])
    if sparks:
        max_sparks = 5
        estimated_max = 800_000
        try:
            with open(config_path) as f:
                scfg = json.load(f)
            max_sparks = scfg.get("max_sparks_display", 5)
            estimated_max = scfg.get("estimated_max_chars", estimated_max)
        except Exception:
            pass
        # Scale with context window size
        if estimated_max > 800_000 and max_sparks == 5:
            scale = min(estimated_max / 800_000, 4.0)
            max_sparks = max(5, int(5 * scale))
        recent = sparks[-max_sparks:]
        lines = []
        for s in recent:
            proj = s.get("project", "unknown")
            text = s.get("text", "")
            if len(text) > 150:
                text = text[:147] + "..."
            lines.append(f"- **{proj}**: {text}")
        sections.append("### Recent Sparks\n\n" + "\n".join(lines))
except Exception:
    pass

if synthesis_findings:
    sections.append(f"### Cross-Machine Patterns\n\n{synthesis_findings}")

if project_notes:
    sections.append(f"### Project Notes ({project})\n\n{project_notes}")

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
