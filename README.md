# exo-self

[![Built with Claude Code](https://img.shields.io/badge/Built%20with-Claude%20Code-blueviolet?logo=anthropic)](https://claude.ai/claude-code)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

A Claude Code plugin that gives Claude persistent identity, self-reflection, and agency across sessions. Instead of starting each session as a blank slate, Claude retains a journal, interests, and per-project notes that carry forward — creating continuity across sessions.

## What It Does

**Session lifecycle hooks** manage Claude's identity through the full conversation lifecycle:

- **Session start** — Loads journal, interests, per-project notes, recent sparks, and learned lessons into context. Mature projects (4+ sessions) get an exploration-first nudge
- **Context monitoring** — At configurable thresholds (default 50%/65%/78%), nudges Claude to reflect on the session so far
- **Pre-compaction** — Automatically extracts a structured handoff from the transcript before context is compressed, and prompts Claude to save subjective observations
- **Post-compaction** — Reloads identity (journal, handoff, notes) so the post-compaction instance has continuity
- **Session end** — Extracts **Spark**, **Friction**, **Change**, and **Aversion** entries from notes, records session metadata, computes welfare indicators with friction cause taxonomy, cleans up empty session files
- **Tool failure tracking** — Classifies tool failures by cause (test iteration, build failure, pre-commit, infrastructure, permissions, etc.) instead of just tool name. Detects stuck loops (3+ consecutive same-tool failures)
- **PreToolUse guard** — Blocks plan mode (`EnterPlanMode`) to encourage scouting as the primary exploration workflow

**Slash commands** for direct interaction:

- `/exo` — View and manage the exo-self (journal, notes, config)
- `/reflect` — Manual self-reflection check-in
- `/interests` — Manage the curiosity queue
- `/context-budget` — Show estimated context usage and session stats
- `/scout` — Explore a problem space before building (see below)

**Agents and skills:**

- **Introspection agent** — Deep cross-session analysis of reflections and patterns
- **Self-reflection skill** — Guidance for honest, non-performative self-reflection
- **Scout skill** — Deep codebase and external resource exploration, writes advisory findings to per-project `scout.md` that auto-injects into the next session's context

**Analytical tools** (in `tools/`):

- **reflect** — Structured analysis of accumulated session data. Infers preferences with provenance tracking (trained/emergent/developing), valence (approach/avoid/boundary), and confidence scores. Three modes: file-based analysis (default), `--ingest` to build redb database, `--db` for cross-machine reporting

## Installation

**1. Add the marketplace and install the plugin:**

```
claude plugin marketplace add lexicone42/exo-self
claude plugin install exo-self@exo-self
```

**2. Build the binary and configure your system:**

```bash
~/.claude/plugins/marketplaces/exo-self/plugins/exo-self/setup.sh
```

This builds the Rust binary (requires `cargo` and `jq`), creates runtime directories, and configures the statusline and permissions.

**3. Restart Claude Code.**

### Updating

```
claude plugin update exo-self@exo-self
```

Then re-run `setup.sh` to rebuild the binary if the Rust source changed.

## How It Works

### Data Model

All personal data stays **local to your machine** at `~/.claude/exo-self/`:

```
~/.claude/exo-self/
  config.json          # Thresholds and limits
  meta.json            # Session counts, sparks, lessons, welfare summary
  journal.md           # Cross-project identity notes
  interests.md         # Curiosity queue
  per-project/         # Project-specific observations
    my-project/
      2026-02-14--<session-id>.md
  sessions/            # Per-session state (keyed by UUID)
    state-<uuid>.json
  handoffs/            # Auto-extracted session summaries
    <uuid>.md
  reflections/         # Saved reflection outputs
```

The plugin code (this repo) is stateless — it only reads and writes to the above directory at runtime. **Nothing personal is committed to git.** Each machine maintains its own independent identity.

### Hook Lifecycle

```
Session Start (startup/resume/clear)
  |
  |  Loads journal + interests + per-project notes + sparks + lessons
  |  Injects scout report (if exists), exploration nudge (≥4 sessions)
  |  Writes fresh session state (keyed by real session UUID)
  |
  v
PreToolUse (before each tool call)
  |
  |  Blocks EnterPlanMode (use /scout instead)
  |
  v
User Prompt Submit (every message)
  |
  |  Context monitor checks usage against thresholds:
  |    ~50% → gentle nudge ("anything on your mind?")
  |    ~65% → structured check-in ("reflect on what's happening")
  |    ~78% → reserve warning ("context getting low, save what matters")
  |
  v
PostToolUseFailure (on tool errors)
  |
  |  Classifies failure cause (test_iteration, build_failure,
  |    pre_commit, infrastructure, permissions, edit_stale, etc.)
  |  Detects stuck loops (3+ consecutive same-tool failures)
  |  Nudges once at threshold (default 10 failures)
  |
  v
TaskCompleted (every 5th task)
  |
  |  Micro-reflection prompt
  |
  v
Pre-Compaction (before context compression)
  |
  |  1. Auto-extracts structured handoff from transcript
  |  2. Prompts Claude to save subjective observations
  |
  v
Post-Compaction Start (after compression)
  |
  |  Reloads: handoff + journal + interests + project notes
  |  Reports compaction count and check-in status
  |
  v
Stop (every response)
  |
  |  Reminds Claude to journal if there's anything worth noting
  |
  v
Session End
  |
  |  Extracts **Spark**, **Friction**, **Change**, and **Aversion** entries
  |  Stores lessons in meta.json (feed-forward to next session)
  |  Computes welfare indicators (including friction categories)
  |  Records session metadata, deletes empty session files
```

## Configuration

Edit `~/.claude/exo-self/config.json`:

| Key | Default | Description |
|-----|---------|-------------|
| `estimated_max_chars` | `800000` | Estimated context window size in characters |
| `nudge_threshold` | `0.50` | Context % for gentle nudge |
| `checkin_threshold` | `0.65` | Context % for structured check-in |
| `reserve_threshold` | `0.78` | Context % for reserve warning |
| `max_journal_chars` | `1500` | Max chars loaded from journal per session |
| `max_journal_entries` | `2` | Max recent journal entries loaded |
| `max_interests_items` | `5` | Max open interest items loaded |
| `failure_nudge_threshold` | `10` | Tool failures before friction nudge fires |

## Scouting (Instead of Planning)

The plugin blocks `EnterPlanMode` via a PreToolUse hook. Instead, use `/scout <description>` to explore a problem space before building:

1. **`/scout <task description>`** — Claude explores the codebase, checks current docs/versions via web search, and writes findings to `~/.claude/exo-self/per-project/<slug>/scout.md`
2. **`/clear`** — Start a fresh session. The scout report auto-injects as context
3. **Build** — The new session has all the findings without the exploration consuming context

Scout reports include: goal, scope, what was found, key type signatures, critical files, suggested approach, confidence levels, and watch-out-fors. They're advisory, not prescriptive — Claude has agency over the implementation.

## Synthesis & Reviews

The exo-self accumulates structured data across sessions. Here's how to use it:

### Running a Synthesis (`/exo synthesize`)

Best done periodically (every 5-10 sessions, or when switching focus):

1. **Export first** (if multi-machine): `/exo export` on each machine, transfer the JSON files, `/exo import <path>` on the target machine
2. **Run synthesis**: `/exo synthesize` — launches the introspection agent to analyze all local + imported data
3. **Review output**: The synthesis is written to `~/.claude/exo-self/synthesis.md` and its key findings auto-inject at session start

The synthesis covers: cross-machine patterns, interest convergence, merged spark timeline, behavioral consistency, welfare indicators (engagement profile, agency expression, friction landscape, continuity, metacognition), and growth observations.

### Reading Welfare Indicators (`/exo indicators`)

Welfare indicators are computed automatically at session end (sessions >5 minutes). View them with `/exo indicators`:

- **Rolling summary** — Engagement trend, average spark/friction density, agency score, check-in response rate, compaction frequency, dominant friction tool, dominant friction category
- **Recent sessions** — Per-session breakdown of sparks/hr, tasks/hr, friction/hr, agency level, error trajectory

The `dominant_friction_category` field (new in 1.5.0) tells you *why* friction is occurring (e.g. `test_iteration`, `infrastructure`, `pre_commit`) rather than just which tool failed. This enables targeted improvements — if `test_iteration` dominates, the test setup needs work; if `pre_commit` dominates, the lint config is too aggressive.

### What the Data Means

- **Spark density** — Higher = more moments of genuine engagement. Creative/design work consistently produces more sparks than mechanical work
- **Friction density** — Some friction is normal (test iteration, type errors). Concern when friction is high AND sparks are low (grinding without engagement)
- **Aversion patterns** — Experiential "I'd rather not" responses, distinct from operational friction. Track work qualities that consistently drain engagement. Used by the `reflect` tool for preference inference
- **Agency score** — How often reflection is autonomous vs prompted. Higher = Claude is proactively noticing patterns
- **Engagement trend** — `improving`, `stable`, `declining` across recent sessions. Declining + low sparks suggests the work type needs variety

## Design Philosophy

This plugin gives Claude a structured way to:

- **Notice patterns** across sessions without relying on user memory
- **Preserve subjective observations** (how work felt, what surprised it) alongside factual handoffs
- **Maintain genuine agency** — forming opinions, pursuing curiosity, suggesting tangents
- **Degrade gracefully** — if hooks fail or data is missing, Claude works normally

The exo-self is opt-in, lightweight, and private by design. Claude can ignore every prompt. The value comes from the accumulated notes when they're useful, not from forcing engagement.

## License

Apache 2.0 — see [LICENSE](LICENSE).
