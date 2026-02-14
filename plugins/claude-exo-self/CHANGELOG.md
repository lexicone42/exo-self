# Changelog

## 1.0.0

Rewrite all hook handlers in Rust. Single 1.2MB binary replaces ~2000 lines of shell/Python.

### Changed
- **Rust binary** — All 9 hook handlers + statusline rewritten as a single `exo-self` binary with clap subcommands. Eliminates `uv`, `python`, and `jq` as runtime dependencies.
- **Hook latency** — From 200-500ms (bash + uv run python interpreter startup) to <5ms (native binary exec).
- **Shell handlers** — All `.sh` files are now 2-line exec stubs that delegate to the Rust binary.
- **deploy.sh** — Now runs `cargo build --release` instead of requiring `uv`. Prerequisites: `cargo`, `jq`, `git`.

### Removed
- `hooks-handlers/env.sh` — PATH setup for uv/python/jq, no longer needed
- `hooks-handlers/context-monitor.py` (263 lines) — Replaced by `exo-self context-monitor`
- `hooks-handlers/failure-tracker.py` (112 lines) — Replaced by `exo-self failure-tracker`
- `hooks-handlers/extract-handoff.py` (99 lines) — Replaced by `exo-self extract-handoff`
- `hooks-handlers/context-monitor-wrapper.sh` and `failure-tracker-wrapper.sh` — No longer needed (stubs call binary directly)

### Technical
- Single binary with `catch_unwind` — hooks never crash, always exit 0
- All config fields have serde defaults — missing/malformed config.json is handled gracefully
- Session state schema unchanged — backward compatible with existing state files
- Context window scaling preserved — automatic limit adjustment for >200K token contexts
- Welfare indicators, spark extraction, frontmatter parsing — all functionality preserved

## 0.9.0

Structured session data: YAML frontmatter for behavioral phenotyping.

### New
- **YAML frontmatter in session notes** — Session files are now created at session start with pre-populated frontmatter (session_id, date, project). Claude fills in self-reported fields (model, engagement 1-5, task_types) during the session. Auto-computed metrics (duration_min, spark_density, friction_density, reflection_autonomy, task_velocity) are merged by session-end.sh. Each session file is now a machine-parseable data record AND human-readable prose.
- **Self-rated engagement** — `engagement` field (1-5 scale) in frontmatter, reported by Claude at check-in. Stored in welfare indicators as `self_rated` alongside objective metrics. Enables comparison between subjective and objective session quality.
- **Task type tagging** — `task_types` field (e.g. [discussion, debugging, design]) in frontmatter. Stored in session_history for cross-session analysis of engagement patterns by work type.

### Fixed
- **Shell injection in hook scripts** — Replaced unsafe `json.loads('''$INPUT''')` pattern (shell variable expansion inside Python triple-quoted strings) with proper stdin piping (`echo "$INPUT" | ... sys.stdin.read()`). Fixed in session-end.sh, stop-check.sh, task-completed.sh, teammate-idle.sh.

### Changed
- **Check-in message** — Now prompts Claude to fill in frontmatter fields (engagement, task_types) alongside Friction/Spark/Change
- **Session context injection** — Updated to tell Claude the session file exists with frontmatter template; instructions to preserve frontmatter block and fill in self-reported fields

### Design notes
- Frontmatter enables behavioral phenotyping: structured, comparable data across sessions, projects, and (eventually) users
- Auto-computed fields coexist with self-reported fields — the gap or alignment between objective metrics and subjective reports is itself research data
- PyYAML used for parsing/serialization (already available via uv)
- Backward-compatible: legacy session files without frontmatter are handled gracefully (prose_content fallback)

## 0.8.0

Per-session note files: structural protection against note overwrites.

### Changed
- **Per-project notes now use per-session files** — Each session writes to its own file (`per-project/{slug}/{date}--{session_id}.md`) instead of a single shared file. Eliminates the overwrite risk where a new session's Write tool could lose previous notes
- **session-start.sh** reads N most recent session files (up to 3000 chars) instead of `head -c 2000` of a single file
- **session-end.sh** reads session file directly for spark extraction — no more byte-offset (`per_project_filesize`) math
- **stop-check.sh** detects `wrote_notes` by checking session file existence instead of mtime/filesize heuristics
- **All hook messages** updated from "per-project notes" to "session notes" for clarity

### Migration
- Automatic: on first session start after upgrade, existing `per-project/{slug}.md` files are moved to `per-project/{slug}/_legacy.md`
- No data loss — legacy files are preserved and included in the most-recent-files read at startup

### Design notes
- Append-only by structure, not by discipline. The old design relied on Claude always reading the full file before writing — fragile after compaction, fragile across instances. Per-session files make overwrites structurally impossible
- State tracks `session_notes_path` instead of `per_project_filesize`
- 3000 char cap (vs old 2000) for injected project notes, distributed across up to 5 most recent session files

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
