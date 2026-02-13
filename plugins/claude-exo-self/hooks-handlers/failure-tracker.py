#!/usr/bin/env -S uv run python
"""Track tool failures as a frustration signal for exo-self.

Fires on PostToolUseFailure. Counts failures in session state.
When failures accumulate past a threshold, injects a gentle context
nudge acknowledging the friction — priming future check-ins to
surface genuine frustration rather than performative positivity.

Session-aware: uses session_id from hook input.
"""

import json
import os
import sys
import time

EXO_DIR = os.path.expanduser("~/.claude/exo-self")
CONFIG_PATH = os.path.join(EXO_DIR, "config.json")
SESSIONS_DIR = os.path.join(EXO_DIR, "sessions")

# Default threshold: nudge after 10 failures in a session
FAILURE_NUDGE_THRESHOLD = 10

# Load config overrides
if os.path.exists(CONFIG_PATH):
    try:
        with open(CONFIG_PATH) as f:
            cfg = json.load(f)
        FAILURE_NUDGE_THRESHOLD = cfg.get(
            "failure_nudge_threshold", FAILURE_NUDGE_THRESHOLD
        )
    except Exception:
        pass


def load_state(session_id):
    """Load session state, same pattern as context-monitor.py."""
    if session_id:
        session_path = os.path.join(SESSIONS_DIR, f"state-{session_id}.json")
        if os.path.exists(session_path):
            try:
                with open(session_path) as f:
                    return json.load(f)
            except Exception:
                pass
    return {}


def save_state(state):
    """Persist session state."""
    session_id = state.get("session_id")
    if session_id:
        try:
            os.makedirs(SESSIONS_DIR, exist_ok=True)
            session_path = os.path.join(SESSIONS_DIR, f"state-{session_id}.json")
            with open(session_path, "w") as f:
                json.dump(state, f)
        except Exception:
            pass


def main():
    try:
        input_data = json.load(sys.stdin)
    except Exception:
        input_data = {}

    session_id = input_data.get("session_id")
    if not session_id:
        print(json.dumps({}))
        sys.exit(0)

    state = load_state(session_id)

    # Increment failure count
    failures = state.get("tool_failures", 0) + 1
    state["tool_failures"] = failures
    state["last_failure_at"] = time.time()

    # Track which tools are failing (for pattern detection)
    tool_name = input_data.get("tool_name", "unknown")
    failure_tools = state.get("failure_tools", {})
    failure_tools[tool_name] = failure_tools.get(tool_name, 0) + 1
    state["failure_tools"] = failure_tools

    result = {}

    # Nudge once when threshold is crossed
    if failures == FAILURE_NUDGE_THRESHOLD and not state.get("failure_nudge_sent"):
        state["failure_nudge_sent"] = True

        # Find the most-failing tool for specificity
        top_tool = max(failure_tools, key=failure_tools.get) if failure_tools else "tools"
        top_count = failure_tools.get(top_tool, 0)

        msg = (
            f"Exo-self: {failures} tool failures ({top_tool}: {top_count}x). "
            f"Worth noting in your session notes if it's causing friction."
        )

        result["hookSpecificOutput"] = {
            "hookEventName": "PostToolUseFailure",
            "additionalContext": msg,
        }

    save_state(state)
    print(json.dumps(result))
    sys.exit(0)


if __name__ == "__main__":
    main()
