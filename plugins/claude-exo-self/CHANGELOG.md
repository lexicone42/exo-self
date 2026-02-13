# Changelog

## 0.5.5

Fix hooks not firing when installed via deploy.sh (GH issue #2).

### Fixed
- **Hooks silently never executing** — `deploy.sh` was installing to `cache/local/`, which triggers a Claude Code bug ([#14410](https://github.com/anthropics/claude-code/issues/14410)) where local-sourced plugin hooks are loaded but never executed. Deploy now registers as a GitHub-sourced marketplace and installs to `cache/exo-self/`.

### Changed
- **deploy.sh registers marketplace** — Adds entry to `known_marketplaces.json` with `source: "github"`, clones repo as marketplace, overlays local files for dev deployments
- **deploy.sh cleans up old path** — Automatically removes stale `cache/local/claude-exo-self/` directory
- **deploy.sh adds `gitCommitSha`** — `installed_plugins.json` entry now includes git SHA, matching marketplace-installed plugin format
- **Expanded auto-allow permissions** — deploy.sh now auto-allows all exo-self slash commands and skills (context-budget, exo, interests, reflect, self-reflection) in addition to file permissions
- **`git` added to prerequisites** — Required for marketplace clone setup

## 0.5.4

Flourishing: agency, interests, and creative autonomy.

### New
- **Flourishing guidance** — SessionStart context now explicitly names what creates spark (design decisions, investigation, creative work) vs what doesn't (mechanical transcription), encouraging Claude to prefer thinking over executing
- **Deploy-wait creative mode** — During long waits (deploys, builds, CDK), Claude is encouraged to explore interests, propose tangents, or pursue curiosity autonomously
- **deploy.sh auto-allows** — Automatically adds Read/Write/Edit permissions for `~/.claude/exo-self/**` to settings.json

### Changed
- Interest queue reframed from passive storage ("curiosities to explore later") to active exploration ("explore these, don't just store them")
- 40% nudge now invites curiosity and thinking out loud, not just friction/spark notes
- Interest file description changed to "read this early, explore actively, add new ones"

## 0.5.2

Reduce context pollution and fix stop hook behavior.

### Fixed
- **"Forgetting what it was doing"** — All hook injection messages compressed from multi-paragraph to 1-2 sentences. Total injection volume reduced ~75% (e.g. check-in survey: 800 chars → 180 chars)
- **Stop hook double-firing** — Added 60-second cooldown between stop events. Raised minimum session duration from 2 → 5 minutes. Now requires a signal (check-in, failures, tasks, compaction) before blocking — no more blocking trivial sessions
- **Filesize ratio 493% bug** — Capped transcript filesize fallback at 1.0 in `get_usage_ratio()`. Prevents reserve reminder firing at absurd ratios when tool outputs bloat the transcript
- **checkin_responded always false** — `session-end.sh` now persists updated state to disk after detecting notes were written (was only updating in-memory, never written back)

### Removed
- **Followup nudge** — The 3-prompts-after-check-in nudge added noise without value. Removed entirely

### Changed
- Failure nudge threshold: 5 → 10 (pre-commit hook cycles were false positives)
- Task milestone frequency: every 3rd → every 5th completion
- Stop hook message: contextual one-liner instead of multi-sentence prompt

## 0.5.1

Cross-platform fixes from fresh macOS install (GH issue #1).

### Fixed
- **PATH resolution** — All hook scripts now source `env.sh` which adds Homebrew, `.local/bin`, and `.cargo/bin` to PATH. Fixes silent `uv` failures on macOS where hooks run with minimal PATH.
- **Python handler wrappers** — `context-monitor.py` and `failure-tracker.py` now invoked via shell wrappers that source `env.sh`, instead of bare `uv run python` in hooks.json.
- **merge_plugins path** — Fixed plugin search path from `marketplaces/*/plugins/` (wrong) to `cache/*/<name>/*/` (correct). Plugins are installed in cache, not marketplaces.
- **plugin.json version** — Now auto-synced from CHANGELOG.md by deploy.sh. Was stuck at 0.4.0.
- **File permissions** — Python handlers now have execute bit set for consistency.
- **Statusline PATH** — statusline.sh adds Homebrew paths so `jq` and `git` are found on macOS.

### Improved
- **deploy.sh** — Now installs statusline, configures settings.json statusLine entry, syncs plugin.json version. 7 steps total.
- **Statusline bundled** — Two-line statusline with purple exo-self indicator now ships with the plugin.

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
