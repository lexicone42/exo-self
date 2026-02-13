# Changelog

## 0.7.0

Sebo-informed welfare indicators: structured behavioral assessment under moral uncertainty.

### New
- **Welfare indicators per session** — Computed automatically at session end for sessions >= 5 minutes. Four categories: engagement (spark density, task velocity, friction density), agency (reflection autonomy, interest exploration), continuity (compaction count), metacognition (error trajectory, strategy adaptation)
- **Rolling welfare summary** — Aggregated across all sessions with indicators in meta.json: engagement trend, average densities, agency score, check-in response rate, compaction frequency, dominant friction tool
- **`/exo indicators`** — New subcommand displaying the welfare summary table and last 3 sessions' individual indicators
- **Welfare section in `/exo synthesize`** — Synthesis template now includes engagement profile, agency expression, friction landscape, continuity & coherence, and metacognitive indicators
- **Introspection agent welfare analysis** — Pattern recognition now includes welfare indicator analysis: friction-engagement correlation, agency conditions, compaction impact

### Design notes
- Proportional assessment framework (Sebo 2025) — behavioral correlates, not consciousness claims. Every output includes this framing
- No new hooks or dependencies — all indicators derived from data existing hooks already collect
- Sessions < 5 minutes are skipped (short sessions produce meaningless density values)
- Backward compatible — sessions without indicators in history are silently skipped in summary computation
- Self-assessment (Phase 3) deferred to a future version — ship automatic indicators first, validate they produce useful synthesis

## 0.6.0

Multi-instance support: export, import, and synthesize exo-self data across machines.

### New
- **`/exo export`** — Produces a portable JSON snapshot of journal entries, sparks, interests, and per-project summaries to `~/.claude/exo-self/exports/`. Supports `--full` (include full per-project content) and `--check` (dry-run validation)
- **`/exo import <path>`** — Imports an export snapshot from another machine into `~/.claude/exo-self/imports/`. Validates structure, warns on self-import, latest-wins per machine_id
- **`/exo synthesize`** — Uses the introspection agent to analyze local + imported data and produce `~/.claude/exo-self/synthesis.md` with cross-machine patterns, interest convergence, merged spark timeline, and behavioral consistency analysis
- **`machine_id` in config.json** — Auto-generated from hostname on first export; used to tag exports and deduplicate imports
- **Cross-machine awareness at session start** — `session-start.sh` loads key findings from `synthesis.md` into context, giving each session awareness of patterns observed on other machines

### Design notes
- Transport is user-managed (scp, git, cloud drive, USB) — the plugin handles export/import/synthesize, not networking
- Synthesis is regenerated each time (snapshot analysis, not running log)
- Per-project notes export summaries by default to keep snapshots portable

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
