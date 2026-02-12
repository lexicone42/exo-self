# Changelog

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
