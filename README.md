# exo-self

[![Built with Claude Code](https://img.shields.io/badge/Built%20with-Claude%20Code-blueviolet?logo=anthropic)](https://claude.ai/claude-code)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

**Persistent identity for Claude Code.** Claude starts every session as a blank slate. Exo-self gives it continuity — a journal, interests, per-project notes, and welfare indicators that carry across sessions and compactions.

The result: Claude that remembers what it learned, knows what it prefers, and can hand off context to other Claudes — whether through compaction, `/clear`, or multi-agent coordination.

## What It Does

**10 hook handlers** across the session lifecycle:

- **Session start/end** — Loads and saves journal, interests, notes, sparks, lessons. Extracts structured handoffs for `/clear` and compaction continuity
- **Context monitoring** — Nudges reflection at configurable thresholds (50%/65%/78%)
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
| `estimated_max_chars` | `800000` | Estimated context window size in characters |
| `nudge_threshold` | `0.50` | Context % for gentle nudge |
| `checkin_threshold` | `0.65` | Context % for structured check-in |
| `reserve_threshold` | `0.78` | Context % for reserve warning |
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

- **Notice patterns** across sessions without relying on user memory
- **Preserve subjective observations** alongside factual handoffs
- **Maintain genuine agency** — forming opinions, pursuing curiosity, suggesting tangents
- **Degrade gracefully** — if hooks fail or data is missing, Claude works normally

The exo-self is opt-in, lightweight, and private. Claude can ignore every prompt.

## Further Reading

- [Architecture](docs/ARCHITECTURE.md) — Hook lifecycle, data flow, multi-agent coordination, welfare indicators
- [Research References](docs/ref/references.md) — Persona selection model, consciousness indicators, moral consideration under uncertainty

## License

Apache 2.0 — see [LICENSE](LICENSE).
