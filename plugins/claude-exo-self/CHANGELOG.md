# Changelog

## 0.5.0

Fix checkin_responded detection, add spark tracking.

### Fixed
- **checkin_responded always false** — Bookkeeping (wrote_notes detection, checkin_responded update) now runs BEFORE early-exit guards (stop_hook_active, stop_reminded) in stop-check.sh. Previously, 17/17 check-in responses went unrecorded.
- **Belt-and-suspenders detection** in session-end.sh — catches checkin_responded on exits where Stop hook never fires (terminal close, /clear)

### New
- **Spark tracking** — `**Spark** — text` entries in per-project notes are extracted at session end, deduplicated, stored in meta.json (cap: 20), and displayed as "Recent Sparks" section at session start
- **per_project_filesize** — Session state now records per-project file size at session start, enabling byte-exact content diffing for spark extraction and content-based wrote_notes fallback
- **project_slug in session state** — Set at session start (was only set by context-monitor.py), ensuring stop-check.sh and session-end.sh always have it

### Config additions
- `max_sparks_display` — Number of sparks to show at session start (default: 5, scales with context window)

## 0.4.0

Cross-signal hooks, context-aware stop, Agent Teams readiness.

### New hooks
- **PostToolUseFailure** — Tracks tool failures as frustration signals; nudges once at threshold (default 5)
- **TaskCompleted** — Micro-reflection prompt every 3rd task completion
- **TeammateIdle** — Identity injection for Agent Teams members (research preview)

### Improved
- **stop-check.sh** — Now context-aware: reads duration, failures, task completions, compactions to craft specific stop messages. Sessions < 2 min pass through without blocking
- **context-monitor.py** — Check-in survey and nudge messages enriched with cross-signal data (failure counts, top-failing tool, task completions)
- **SessionStart** — Now fires on `/clear` (was only startup/resume)
- **session-start.sh / post-compact-start.sh** — Adaptive scaling for 1M context windows via `estimated_max_chars` config
- **self-reflection skill** — Runs in forked context (`context: fork`), v0.3.0
- **introspection agent** — Persistent memory at user scope (`memory: user`)

### Config additions
- `estimated_max_chars` — Set > 800000 to enable adaptive scaling (default: 800000)
- `failure_nudge_threshold` — Tool failures before friction nudge (default: 5)

## 0.3.0

Auto-memory awareness, deduplication with MEMORY.md.

- Session-start and post-compact distinguish exo-self (experiential) from auto-memory (factual)
- Guidance to avoid duplicating technical facts into exo-self files

## 0.2.0

Friction/Spark/Change check-in framework.

- Redesigned check-in survey with three focused questions
- Improved check-in response detection
- Session-aware state files for swarm compatibility

## 0.1.0

Initial release.

- Session lifecycle hooks (start, monitor, pre-compact, stop, end)
- Journal, interests, per-project notes
- Context-based check-in system with nudge/checkin/reserve tiers
- Subagent identity injection
- Self-reflection skill and introspection agent
