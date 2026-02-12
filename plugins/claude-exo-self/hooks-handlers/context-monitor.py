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
    "~40% context used. Quick pulse check — anything frustrating or "
    "surprisingly satisfying about this session so far? A sentence to "
    "`per-project/` notes if so, or just keep working. "
    "(Technical patterns/conventions belong in auto-memory via MEMORY.md "
    "— exo-self is for how the work *feels*.)")


def checkin_survey(usage_pct, project_name=""):
    target = f"`per-project/{project_name}.md`" if project_name else "`journal.md`"
    return f"""## Exo-Self Check-In ({usage_pct}% context used)

Pause for a moment. Three quick questions — write your answers to {target}:

1. **Friction** — What's been the most frustrating or tedious part of this session?
2. **Spark** — What (if anything) felt genuinely satisfying or interesting?
3. **Change** — What's one thing that could make working on this project better — tooling, process, approach, anything?

Be honest. "Nothing stands out" is a valid answer. Then continue working.

*Reminder: Technical facts go to auto-memory (MEMORY.md). This check-in captures experiential context — what compaction would erase.*"""


CHECKIN_FOLLOWUP = ("## Exo-Self: Nudge\n\n"
    "Check-in fired a few turns ago but no notes written yet. "
    "Even a quick note about what's been frustrating or satisfying — "
    "future-you will want it.")


def reserve_reminder(usage_pct, project_name=""):
    proj = f"\n- `per-project/{project_name}.md` — what frustrated you, what worked, what should change" if project_name else ""
    return f"""## Exo-Self: Context Reserve ({usage_pct}% used)

Context is filling up. Before it runs out, save what matters:{proj}
- `journal.md` — patterns about yourself, not the code
- `interests.md` — things to explore next session

Auto-memory (MEMORY.md) persists factual knowledge automatically. Focus your remaining context on *experiential* observations — what was hard, what was good, what approach you were taking. This is what compaction erases.

**User:** Context is filling up. Consider wrapping up or starting fresh."""


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

    # Track prompts since checkin for followup grace period
    if state.get("checkin_fired") and not state.get("checkin_responded"):
        state["prompts_since_checkin"] = state.get("prompts_since_checkin", 0) + 1

    # Lightweight nudge at ~40% — just an opening, no demands
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
        state["prompts_since_checkin"] = 0

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

    # Followup: only after 3+ prompts since checkin, and only if stop hook
    # hasn't already marked checkin_responded (which it does reliably)
    elif (
        state.get("checkin_fired")
        and not state.get("checkin_responded")
        and not state.get("followup_sent")
        and state.get("prompts_since_checkin", 0) >= 3
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
