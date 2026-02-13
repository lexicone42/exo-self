#!/usr/bin/env bash
source "$(dirname "$0")/env.sh"
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
        # Persist updated state so meta session_history reads the correct value
        try:
            with open(state_path, 'w') as f:
                json.dump(state, f)
        except Exception:
            pass

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
                    # Multi-line spark extraction: capture from **Spark** marker
                    # until the next **bold** marker, double newline, or end of string
                    spark_pattern = r'\*\*Spark\*\*\s*[-\u2014]\s*(.+?)(?=\n\*\*|\n\n|$)'
                    sparks_found = re.findall(spark_pattern, new_content, re.DOTALL)

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

    # --- Welfare indicator computation (Sebo proportional assessment) ---
    indicators = None
    if duration_min >= 5:
        hours = duration_min / 60.0
        sparks_this_session = len(sparks_found) if 'sparks_found' in dir() else 0
        task_completions = state.get('task_completions', 0)
        tool_failures = state.get('tool_failures', 0)
        failure_tools = state.get('failure_tools', {})

        # Engagement
        spark_density = round(sparks_this_session / hours, 2) if hours > 0 else 0
        task_velocity = round(task_completions / hours, 2) if hours > 0 else 0
        friction_density = round(tool_failures / hours, 2) if hours > 0 else 0

        # Agency — reflection_autonomy: did notes get written before or after check-in?
        checkin_fired_at = state.get('checkin_fired_at', 0)
        reflection_autonomy = 'none'
        if project_slug:
            proj_path = os.path.join(exo_dir, 'per-project', f'{project_slug}.md')
            journal_path = os.path.join(exo_dir, 'journal.md')
            start_size = state.get('per_project_filesize', 0)
            wrote_notes = False
            notes_mtime = 0
            for check_path in [proj_path, journal_path]:
                if os.path.exists(check_path):
                    try:
                        mt = os.path.getmtime(check_path)
                        if mt > session_start:
                            wrote_notes = True
                            notes_mtime = max(notes_mtime, mt)
                    except OSError:
                        pass
            if wrote_notes:
                if checkin_fired_at and notes_mtime < checkin_fired_at:
                    reflection_autonomy = 'autonomous'
                elif checkin_fired_at:
                    reflection_autonomy = 'prompted'
                else:
                    reflection_autonomy = 'autonomous'

        # Agency — interest exploration
        interests_path = os.path.join(exo_dir, 'interests.md')
        interest_explored = False
        if os.path.exists(interests_path) and session_start > 0:
            try:
                interest_explored = os.path.getmtime(interests_path) > session_start
            except OSError:
                pass

        # Agency — autonomous sparks (sparks that appeared before check-in)
        autonomous_sparks = sparks_this_session  # all sparks if no check-in
        # (Spark extraction happens at session end from notes, so timing is
        # approximate — we count all sparks as autonomous if no check-in fired)

        # Metacognition — compare friction to previous session
        prev_indicators = None
        for h in reversed(meta.get('session_history', [])):
            if 'welfare_indicators' in h:
                prev_indicators = h['welfare_indicators']
                break

        error_trajectory = 'stable'
        strategy_adaptation = False
        if prev_indicators:
            prev_friction = prev_indicators.get('engagement', {}).get('friction_density', 0)
            if prev_friction > 0 and friction_density > 0:
                ratio = friction_density / prev_friction
                if ratio < 0.7:
                    error_trajectory = 'improving'
                elif ratio > 1.5:
                    error_trajectory = 'worsening'

            prev_dominant = prev_indicators.get('_dominant_failure_tool', '')
            dominant_now = max(failure_tools, key=failure_tools.get) if failure_tools else ''
            if prev_dominant and dominant_now and prev_dominant != dominant_now:
                strategy_adaptation = True

        dominant_failure_tool = max(failure_tools, key=failure_tools.get) if failure_tools else ''

        indicators = {
            'engagement': {
                'spark_density': spark_density,
                'task_velocity': task_velocity,
                'friction_density': friction_density,
                'checkin_responded': state.get('checkin_responded', False),
            },
            'agency': {
                'reflection_autonomy': reflection_autonomy,
                'interest_explored': interest_explored,
                'autonomous_sparks': autonomous_sparks,
            },
            'continuity': {
                'compaction_count': state.get('compactions', 0),
            },
            'metacognition': {
                'error_trajectory': error_trajectory,
                'strategy_adaptation': strategy_adaptation,
            },
            '_dominant_failure_tool': dominant_failure_tool,
        }

    # Track session history (keep last 10)
    history = meta.get('session_history', [])
    entry = {
        'session_id': session_id or state.get('session_id', ''),
        'ended': datetime.datetime.now().isoformat(),
        'reason': reason,
        'duration_min': duration_min,
        'checkin_fired': state.get('checkin_fired', False),
        'checkin_responded': state.get('checkin_responded', False),
        'compactions': state.get('compactions', 0),
    }
    if indicators:
        entry['welfare_indicators'] = indicators
    history.append(entry)
    meta['session_history'] = history[-10:]

    # --- Rolling welfare summary across all sessions with indicators ---
    sessions_with_indicators = [h for h in meta['session_history'] if 'welfare_indicators' in h]
    if sessions_with_indicators:
        n = len(sessions_with_indicators)
        avg_spark = round(sum(h['welfare_indicators']['engagement']['spark_density'] for h in sessions_with_indicators) / n, 2)
        avg_friction = round(sum(h['welfare_indicators']['engagement']['friction_density'] for h in sessions_with_indicators) / n, 2)

        # Agency score: fraction of sessions with autonomous reflection
        agency_vals = [h['welfare_indicators']['agency']['reflection_autonomy'] for h in sessions_with_indicators]
        agency_score = round(agency_vals.count('autonomous') / n, 2)

        # Check-in response rate
        checkin_sessions = [h for h in sessions_with_indicators if h.get('checkin_fired')]
        checkin_rate = round(sum(1 for h in checkin_sessions if h.get('checkin_responded')) / len(checkin_sessions), 2) if checkin_sessions else None

        # Compaction frequency: fraction of sessions with compactions
        compaction_freq = round(sum(1 for h in sessions_with_indicators if h['welfare_indicators']['continuity']['compaction_count'] > 0) / n, 2)

        # Engagement trend: last 3 vs previous 3 on spark_density
        engagement_trend = 'insufficient_data'
        if n >= 4:
            recent_3 = sessions_with_indicators[-3:]
            prev_group = sessions_with_indicators[-6:-3] if n >= 6 else sessions_with_indicators[:-3]
            if prev_group:
                recent_avg = sum(h['welfare_indicators']['engagement']['spark_density'] for h in recent_3) / len(recent_3)
                prev_avg = sum(h['welfare_indicators']['engagement']['spark_density'] for h in prev_group) / len(prev_group)
                if prev_avg > 0:
                    ratio = recent_avg / prev_avg
                    if ratio > 1.3:
                        engagement_trend = 'increasing'
                    elif ratio < 0.7:
                        engagement_trend = 'decreasing'
                    else:
                        engagement_trend = 'stable'
                else:
                    engagement_trend = 'increasing' if recent_avg > 0 else 'stable'

        # Dominant friction tool across all sessions
        all_tools = {}
        for h in sessions_with_indicators:
            tool = h['welfare_indicators'].get('_dominant_failure_tool', '')
            if tool:
                all_tools[tool] = all_tools.get(tool, 0) + 1
        dominant_friction_tool = max(all_tools, key=all_tools.get) if all_tools else ''

        summary = {
            'computed_at': datetime.datetime.now().isoformat(),
            'sessions_analyzed': n,
            'engagement_trend': engagement_trend,
            'avg_spark_density': avg_spark,
            'avg_friction_density': avg_friction,
            'agency_score': agency_score,
            'compaction_frequency': compaction_freq,
            'dominant_friction_tool': dominant_friction_tool,
        }
        if checkin_rate is not None:
            summary['checkin_response_rate'] = checkin_rate
        meta['welfare_summary'] = summary

    with open(meta_path, 'w') as f:
        json.dump(meta, f, indent=2)
except Exception:
    pass
" 2>/dev/null

# SessionEnd can't block or return decisions — just exit clean
echo '{}'
exit 0
