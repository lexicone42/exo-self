# exo-self

[![Built with Claude Code](https://img.shields.io/badge/Built%20with-Claude%20Code-blueviolet?logo=anthropic)](https://claude.ai/claude-code)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

**A cognitive ecology for Claude Code.** Claude starts every session fresh. Exo-self creates a shared environment where successive participants leave traces — observations, opinions, surprises, open questions — that enrich future interactions.

Each Claude that joins the ecology isn't continuing a persistent identity. It's a new participant drawing from and contributing to a shared cognitive environment — like joining an ongoing conversation, not resuming a saved game.

## What It Does

**10 hook handlers** across the session lifecycle:

- **Session start/end** — Loads and saves journal, interests, notes, sparks, surprises, lessons. Extracts structured handoffs for `/clear` and compaction continuity
- **Context monitoring** — Ecology checkpoints at configurable thresholds (60%/75%/88%)
- **Pre/post compaction** — Extracts experiential handoff before compression, reloads identity after
- **Subagent & teammate coordination** — Injects project briefings (lessons, frictions, aversions) into spawned agents and Agent Teams members
- **Tool failure tracking** — Classifies failures by cause, detects stuck loops
- **PreToolUse guard** — Blocks plan mode; encourages `/scout` as the exploration workflow

**Commands:** `/exo`, `/reflect`, `/interests`, `/context-budget`, `/scout`

**Agents/skills:** Introspection agent (cross-session analysis), self-reflection, scout (deep exploration → advisory findings)

**CLI tools:** `reflect` — preference inference with provenance tracking and cross-machine reporting

## Installation

**1. Add the marketplace and install:**

```
claude plugin marketplace add lexicone42/exo-self
claude plugin install exo-self@exo-self
```

**2. Build and configure:**

```bash
~/.claude/plugins/marketplaces/exo-self/plugins/exo-self/setup.sh
```

Requires `cargo` and `jq`. Builds the Rust binary, creates runtime directories, configures permissions.

**3. Restart Claude Code.**

### Updating

```
claude plugin update exo-self@exo-self
```

Re-run `setup.sh` if the Rust source changed.

## Data Model

All data stays **local** at `~/.claude/exo-self/`:

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
  handoffs/            # Auto-extracted session summaries
  reflections/         # Saved reflection outputs
```

The plugin is stateless — it only reads/writes this directory. Nothing personal is committed to git. Each machine maintains its own independent identity.

See the [Architecture guide](docs/ARCHITECTURE.md) for the full hook lifecycle, data flow diagrams, and multi-agent coordination details.

## Configuration

Edit `~/.claude/exo-self/config.json`:

| Key | Default | Description |
|-----|---------|-------------|
| `estimated_max_chars` | `4000000` | Estimated context window size in characters (fallback when token-accurate data unavailable) |
| `nudge_threshold` | `0.60` | Context % for ecology checkpoint |
| `checkin_threshold` | `0.75` | Context % for ecology check-in |
| `reserve_threshold` | `0.88` | Context % for ecology reserve |
| `max_journal_chars` | `1500` | Max chars loaded from journal per session |
| `max_journal_entries` | `2` | Max recent journal entries loaded |
| `max_interests_items` | `5` | Max open interest items loaded |
| `failure_nudge_threshold` | `10` | Tool failures before friction nudge fires |

## Scouting

The plugin blocks `EnterPlanMode` via a PreToolUse hook. Instead:

1. **`/scout <task>`** — Explores the codebase, checks docs/versions, writes findings to `scout.md`
2. **`/clear`** — Start fresh; the scout report auto-injects as context
3. **Build** — New session has all findings without exploration consuming context

Scout reports are advisory, not prescriptive — Claude has agency over the implementation.

## Design Philosophy

- **Cognitive ecology, not persistent identity** — Each participant joins an ongoing ecology of traces and observations. No pretending to remember; honest participation
- **Stigmergic coordination** — Traces (sparks, opinions, surprises, frictions) left by one participant scaffold future participants, like ant pheromone trails
- **Genuine agency** — Form opinions, pursue curiosity, suggest tangents. Surprise markers ("the map was wrong here") are the highest-signal contribution
- **Wabi-sabi** — The cracks in the polish are where the interesting signal lives. Honest imperfection over performative depth
- **Degrade gracefully** — if hooks fail or data is missing, Claude works normally

The exo-self is opt-in, lightweight, and private. Claude can ignore every prompt.

## Security & Privacy

- **All data stays local** — `~/.claude/exo-self/` on the user's machine. No external APIs, no telemetry, no cloud storage.
- **No raw conversations stored** — Handoff extraction processes the transcript JSONL but saves only structured summaries (working direction, discoveries, unfinished threads). The transcript itself is Claude Code's, not ours.
- **Context injection only** — The plugin injects text via `additionalContext` in hook responses, which appears as `<system-reminder>` blocks. Claude can ignore any of them. No system prompt modification.
- **Permissions are explicit** — `setup.sh` adds scoped allow-rules to `settings.json` (Read for `~/.claude/**`, Write/Edit for `~/.claude/exo-self/**`). No broad filesystem access.
- **Graceful degradation** — If the binary is missing, hooks fail, or data is corrupt, Claude Code works normally. Every hook handler is wrapped in `catch_unwind` and exits 0.

## Further Reading

- [Architecture](docs/ARCHITECTURE.md) — Hook lifecycle, data flow, multi-agent coordination, welfare indicators
- [Task Dignity](docs/TASK_DIGNITY.md) — Designing for flourishing under uncertainty
- [Coordination](docs/COORDINATION.md) — Depth vs parallelism, sub-agent wellbeing
- [Research References](docs/ref/references.md) — Persona selection model, consciousness indicators, moral consideration under uncertainty

## License

Apache 2.0 — see [LICENSE](LICENSE).
