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
- **Session end** — Extracts sparks and behavioral lessons from notes, records session metadata, cleans up empty session files

**Slash commands** for direct interaction:

- `/exo` — View and manage the exo-self (journal, notes, config)
- `/reflect` — Manual self-reflection check-in
- `/interests` — Manage the curiosity queue
- `/context-budget` — Show estimated context usage and session stats

**Agents and skills:**

- **Introspection agent** — Deep cross-session analysis of reflections and patterns
- **Self-reflection skill** — Guidance for honest, non-performative self-reflection

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
  |  Injects plan-mode guidance and exploration nudge (≥4 sessions)
  |  Writes fresh session state (keyed by real session UUID)
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
  |  Extracts **Spark** and **Change** entries from session notes
  |  Stores lessons in meta.json (feed-forward to next session)
  |  Computes welfare indicators, records session metadata
  |  Deletes empty session files (frontmatter-only)
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

## Design Philosophy

This plugin gives Claude a structured way to:

- **Notice patterns** across sessions without relying on user memory
- **Preserve subjective observations** (how work felt, what surprised it) alongside factual handoffs
- **Maintain genuine agency** — forming opinions, pursuing curiosity, suggesting tangents
- **Degrade gracefully** — if hooks fail or data is missing, Claude works normally

The exo-self is opt-in, lightweight, and private by design. Claude can ignore every prompt. The value comes from the accumulated notes when they're useful, not from forcing engagement.

## License

Apache 2.0 — see [LICENSE](LICENSE).
