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
import json, os, sys, time, datetime, re

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

# --- Belt-and-suspenders: detect checkin_responded if stop-check.sh missed it ---
if state.get('checkin_fired') and not state.get('checkin_responded') and session_start > 0:
    journal_path = os.path.join(exo_dir, 'journal.md')
    project_slug = state.get('project_slug', '')
    wrote_notes = False

    # mtime-based detection
    if os.path.exists(journal_path):
        try:
            wrote_notes = os.path.getmtime(journal_path) > session_start
        except OSError:
            pass

    if not wrote_notes and project_slug:
        proj_path = os.path.join(exo_dir, 'per-project', f'{project_slug}.md')
        if os.path.exists(proj_path):
            try:
                wrote_notes = os.path.getmtime(proj_path) > session_start
            except OSError:
                pass

    # Content-based fallback
    if not wrote_notes and project_slug:
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

    if wrote_notes:
        state['checkin_responded'] = True

# Update meta
try:
    meta = {}
    if os.path.exists(meta_path):
        with open(meta_path) as f:
            meta = json.load(f)

    meta['last_session_end'] = datetime.datetime.now().isoformat()
    meta['last_session_reason'] = reason
    meta['last_session_duration_min'] = duration_min

    # --- Spark extraction from new per-project content ---
    project_slug = state.get('project_slug', '')
    if project_slug and session_start > 0:
        proj_path = os.path.join(exo_dir, 'per-project', f'{project_slug}.md')
        if os.path.exists(proj_path):
            try:
                start_size = state.get('per_project_filesize', 0)
                with open(proj_path) as f:
                    f.seek(start_size)
                    new_content = f.read()

                if new_content:
                    spark_pattern = r'\*\*Spark\*\*\s*[-\u2014]\s*(.+?)(?:\n|$)'
                    sparks_found = re.findall(spark_pattern, new_content)

                    if sparks_found:
                        existing_sparks = meta.get('sparks', [])

                        for spark_text in sparks_found:
                            spark_text = spark_text.strip()
                            if not spark_text:
                                continue

                            # Deduplicate by (text[:100].lower(), project)
                            dedup_key = (spark_text[:100].lower(), project_slug)
                            is_dup = any(
                                (s.get('text', '')[:100].lower(), s.get('project', '')) == dedup_key
                                for s in existing_sparks
                            )
                            if not is_dup:
                                existing_sparks.append({
                                    'text': spark_text,
                                    'project': project_slug,
                                    'timestamp': datetime.datetime.now().isoformat(),
                                    'session_id': session_id or state.get('session_id', ''),
                                })

                        # Cap at 20 entries
                        meta['sparks'] = existing_sparks[-20:]
            except Exception:
                pass

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
