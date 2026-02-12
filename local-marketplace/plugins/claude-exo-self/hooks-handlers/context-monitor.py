#!/usr/bin/env -S uv run python
"""Context monitor for exo-self plugin.

Fires on every UserPromptSubmit. Checks transcript size to estimate
context usage. At CHECKIN_THRESHOLD, injects a check-in survey.
At RESERVE_THRESHOLD, reminds Claude that remaining context is
reserved for its own interests.

Uses transcript_path from the hook input (provided by Claude Code)
instead of guessing the transcript location.

Session-aware: uses session_id from hook input to read/write
session-specific state files in sessions/ for swarm compatibility.
Falls back to shared .context-monitor-state.json if no session_id.
"""

import json
import os
import sys
import time

# Read config
EXO_DIR = os.path.expanduser("~/.claude/exo-self")
CONFIG_PATH = os.path.join(EXO_DIR, "config.json")
META_PATH = os.path.join(EXO_DIR, "meta.json")
SESSIONS_DIR = os.path.join(EXO_DIR, "sessions")

# Defaults — conservative estimate for 200K token context
ESTIMATED_MAX_CHARS = 800_000
NUDGE_THRESHOLD = 0.40     # Lightweight "anything on your mind?" at ~40%
CHECKIN_THRESHOLD = 0.60   # Fire check-in at ~60%
RESERVE_THRESHOLD = 0.80   # Reserve reminder at ~80%

# Load config overrides
if os.path.exists(CONFIG_PATH):
    try:
        with open(CONFIG_PATH) as f:
            cfg = json.load(f)
        ESTIMATED_MAX_CHARS = cfg.get("estimated_max_chars", ESTIMATED_MAX_CHARS)
        NUDGE_THRESHOLD = cfg.get("nudge_threshold", NUDGE_THRESHOLD)
        CHECKIN_THRESHOLD = cfg.get("checkin_threshold", CHECKIN_THRESHOLD)
        RESERVE_THRESHOLD = cfg.get("reserve_threshold", RESERVE_THRESHOLD)
    except Exception:
        pass

# Fallback state path (shared, for backward compat)
SHARED_STATE_PATH = os.path.join(EXO_DIR, ".context-monitor-state.json")

CONTEXT_WINDOW_PATH = os.path.join(EXO_DIR, ".context-window.json")


def load_state(session_id):
    """Load monitor state for this session.

    Always uses session-specific file keyed by the hook's session_id.
    Creates a fresh state if no file exists for this session — never
    falls back to the shared state, which may contain stale data from
    a different concurrent session.
    """
    if session_id:
        session_path = os.path.join(SESSIONS_DIR, f"state-{session_id}.json")
        if os.path.exists(session_path):
            try:
                with open(session_path) as f:
                    return json.load(f)
            except Exception:
                pass
    # No session-specific state exists — start fresh
    return {
        "nudge_fired": False,
        "checkin_fired": False,
        "reserve_fired": False,
        "checkin_responded": False,
        "session_start": time.time(),
    }


def save_state(state):
    """Persist monitor state to session-specific file only.

    The shared state file (.context-monitor-state.json) is no longer written
    here — it caused race conditions when multiple sessions were active.
    Each session's state is isolated by session_id.
    """
    try:
        session_id = state.get("session_id")
        if session_id:
            os.makedirs(SESSIONS_DIR, exist_ok=True)
            session_path = os.path.join(SESSIONS_DIR, f"state-{session_id}.json")
            with open(session_path, "w") as f:
                json.dump(state, f)
    except Exception:
        pass


def get_usage_ratio(input_data):
    """Get context usage ratio, preferring accurate token data from statusline.

    Priority:
    1. Statusline-written .context-window.json (token-accurate, from Claude Code API)
    2. Transcript file size / estimated max chars (rough approximation)

    NOTE: Session ID matching between hooks and statusline is intentionally
    disabled. Claude Code gives hooks short 12-char hex IDs but statusline
    gets full UUIDs — different formats that can't be correlated. The 60s
    freshness window is sufficient to prevent stale cross-session reads.
    If swarm support is needed, the statusline should write per-session files.
    """
    if os.path.exists(CONTEXT_WINDOW_PATH):
        try:
            with open(CONTEXT_WINDOW_PATH) as f:
                ctx = json.load(f)
            updated_at = ctx.get("updated_at", 0)
            if time.time() - updated_at < 120:
                pct = ctx.get("used_percentage") or ctx.get("usage_pct")
                if pct is not None:
                    return float(pct) / 100.0, "tokens"
        except Exception:
            pass

    # Fallback: transcript file size
    transcript_path = input_data.get("transcript_path")
    if transcript_path and ESTIMATED_MAX_CHARS > 0:
        try:
            size = os.path.getsize(transcript_path)
            return size / ESTIMATED_MAX_CHARS, "filesize"
        except OSError:
            pass

    return 0.0, "none"


def project_slug(cwd):
    """Derive a stable project slug from cwd for per-project notes.

    Uses the last two path components for uniqueness without being unwieldy.
    e.g. /datar/workspace/my-project -> workspace--my-project
    """
    if not cwd:
        return ""
    parts = cwd.rstrip("/").split("/")
    # Use last 2 components joined by -- for reasonable uniqueness
    slug_parts = parts[-2:] if len(parts) >= 2 else parts[-1:]
    return "--".join(slug_parts)


NUDGE_MSG = ("## Exo-Self: Quick Nudge\n\n"
    "~40% context used. Anything on your mind — observations, tangents, "
    "something you'd do differently? One sentence to `per-project/` notes "
    "if so, or just keep working.")


def checkin_survey(usage_pct, project_name=""):
    target = f"`per-project/{project_name}.md`" if project_name else "`journal.md`"
    return f"""## Exo-Self Check-In ({usage_pct}% context used)

One sentence: **what's the most useful thing you could tell your next instance about this session?**

Write it to {target}, then continue working."""


CHECKIN_FOLLOWUP = ("## Exo-Self: Journal Reminder\n\n"
    "Check-in fired but no journal entry written. One sentence to "
    "your per-project notes — what should your next instance know?")


def reserve_reminder(usage_pct, project_name=""):
    proj = f"\n- `per-project/{project_name}.md` — project observations" if project_name else ""
    return f"""## Exo-Self: Context Reserve ({usage_pct}% used)

Save state before context runs out:{proj}
- `journal.md` — cross-project identity patterns
- `interests.md` — things to explore next session

Session handoff will be auto-saved at compaction. Focus on *subjective* observations only.

**User:** Context is filling up. Consider wrapping up or starting fresh."""


def check_notes_modified_since(timestamp, project_slug=""):
    """Check if journal or per-project notes were modified after the given timestamp."""
    # Check journal
    journal_path = os.path.join(EXO_DIR, "journal.md")
    try:
        if os.path.exists(journal_path) and os.path.getmtime(journal_path) > timestamp:
            return True
    except OSError:
        pass

    # Check per-project notes
    if project_slug:
        proj_path = os.path.join(EXO_DIR, "per-project", f"{project_slug}.md")
        try:
            if os.path.exists(proj_path) and os.path.getmtime(proj_path) > timestamp:
                return True
        except OSError:
            pass

    return False


def main():
    try:
        # Read input from stdin (hook protocol provides session_id, transcript_path, etc.)
        input_data = json.load(sys.stdin)
    except Exception:
        input_data = {}

    # Use session_id from hook input to load the correct session state
    hook_session_id = input_data.get("session_id")
    state = load_state(hook_session_id)

    # Store session_id in state if not already there
    if hook_session_id and not state.get("session_id"):
        state["session_id"] = hook_session_id

    # Derive project slug from cwd for per-project features
    cwd = input_data.get("cwd", "")
    proj = project_slug(cwd)
    if proj and not state.get("project_slug"):
        state["project_slug"] = proj

    usage_ratio, source = get_usage_ratio(input_data)

    if usage_ratio == 0.0:
        print(json.dumps({}))
        sys.exit(0)

    usage_pct = round(usage_ratio * 100)
    result = {}

    def inject_context(msg):
        """Route message into Claude's context via hookSpecificOutput.additionalContext.

        systemMessage goes to the user's UI — additionalContext is what actually
        gets injected into Claude's conversation context for UserPromptSubmit hooks.
        """
        result["hookSpecificOutput"] = {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": msg,
        }

    # Always check if journal/notes were modified after checkin (regardless of other state)
    if state.get("checkin_fired") and not state.get("checkin_responded"):
        checkin_time = state.get("checkin_fired_at", 0)
        if checkin_time and check_notes_modified_since(checkin_time, proj):
            state["checkin_responded"] = True

    # Lightweight nudge at ~25% — just an opening, no demands
    if not state.get("nudge_fired") and usage_ratio >= NUDGE_THRESHOLD and usage_ratio < CHECKIN_THRESHOLD:
        inject_context(NUDGE_MSG)
        state["nudge_fired"] = True

    # Check if we should fire the check-in survey
    elif not state.get("checkin_fired") and usage_ratio >= CHECKIN_THRESHOLD:
        inject_context(checkin_survey(usage_pct, proj))
        state["checkin_fired"] = True
        state["checkin_fired_at"] = time.time()
        state["checkin_at_ratio"] = round(usage_ratio, 3)
        state["checkin_source"] = source  # track whether we used tokens or filesize

        # Update meta stats
        try:
            if os.path.exists(META_PATH):
                with open(META_PATH) as f:
                    meta = json.load(f)
                meta["total_checkins"] = meta.get("total_checkins", 0) + 1
                with open(META_PATH, "w") as f:
                    json.dump(meta, f, indent=2)
        except Exception:
            pass

    # If check-in fired but no journal write yet, send a one-time followup
    elif (
        state.get("checkin_fired")
        and not state.get("checkin_responded")
        and not state.get("followup_sent")
    ):
        state["followup_sent"] = True
        inject_context(CHECKIN_FOLLOWUP)

    # Check if we should fire the reserve reminder
    elif not state.get("reserve_fired") and usage_ratio >= RESERVE_THRESHOLD:
        inject_context(reserve_reminder(usage_pct, proj))
        state["reserve_fired"] = True
        state["reserve_at_ratio"] = round(usage_ratio, 3)

    save_state(state)
    print(json.dumps(result))
    sys.exit(0)


if __name__ == "__main__":
    main()
