# Architecture

How exo-self connects to Claude Code, where data lives, and how information flows.

## The Big Picture

```
┌─────────────────────────────────────────────────────────────────┐
│  Claude Code                                                    │
│                                                                 │
│  ┌───────────┐                   ┌──────────────────────────┐   │
│  │  Claude   │ ──hook events───> │ Hook Handlers (shell)    │   │
│  │  (model)  │ <──ctx injection─ │ → exo-self binary        │   │
│  │           │                   │ (Rust, single binary)    │   │
│  └───────────┘                   └──────────────┬───────────┘   │
│       │                                         │               │
│       │ spawns                                  │ reads/writes  │
│       ▼                                         ▼               │
│  ┌───────────┐                   ┌──────────────────────────┐   │
│  │ Subagents │ ──hook events───> │ ~/.claude/exo-self/      │   │
│  │ Teammates │ <──proj briefing─ │ (journal, notes, state)  │   │
│  └───────────┘                   └──────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**Key principle:** The plugin code is stateless. All persistent data lives in `~/.claude/exo-self/`. The plugin reads from and writes to that directory at runtime. Nothing personal is committed to git.

## How Hooks Work

Claude Code fires **hook events** at specific lifecycle points. Exo-self registers handlers for 10 of the 14 available events. Each handler is a thin shell script that delegates to a single Rust binary (`~/.claude/bin/exo-self`) via subcommand.

### The Shell → Rust Delegation Pattern

```
hooks.json                          Hook Handlers              Binary
─────────                          ──────────────             ──────
SessionStart event  ──>  session-start.sh  ──>  exo-self session-start
UserPromptSubmit    ──>  context-monitor-wrapper.sh  ──>  exo-self context-monitor
PostToolUseFailure  ──>  failure-tracker-wrapper.sh  ──>  exo-self failure-tracker
PreCompact          ──>  pre-compact.sh  ──>  exo-self pre-compact
Stop                ──>  stop-check.sh  ──>  exo-self stop-check
PreToolUse          ──>  pre-tool-use.sh  ──>  exo-self pre-tool-use
SubagentStart       ──>  subagent-start.sh  ──>  exo-self subagent-start
TaskCompleted       ──>  task-completed.sh  ──>  exo-self task-completed
TeammateIdle        ──>  teammate-idle.sh  ──>  exo-self teammate-idle
SessionEnd          ──>  session-end.sh  ──>  exo-self session-end
```

**Why shell wrappers?** Claude Code expects hook commands to be shell scripts. Each wrapper is 2-5 lines: source a shared guard (`_common.sh`), set the subcommand, exec the binary. The shared guard auto-builds the binary if it's missing (requires `cargo`).

### Hook Input/Output Protocol

Claude Code passes hook input as **JSON on stdin**. The binary reads it, does its work, and returns JSON on stdout:

```
stdin (from Claude Code):
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "transcript_path": "/path/to/transcript.jsonl",
  "trigger": "startup",      // or "compact", "clear", etc.
  "tool_name": "Edit",       // for PreToolUse
  "error": "...",             // for PostToolUseFailure
  "agent_type": "Plan"        // for SubagentStart
}

stdout (back to Claude Code):
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "## Exo-Self\n\nYour journal entries..."
  }
}
```

The `additionalContext` string gets injected into Claude's conversation as a `<system-reminder>` block. This is how exo-self injects identity, nudges, and project briefings into Claude's context.

### Hook Event Reference

| Event | When it fires | What exo-self does | Matcher |
|-------|--------------|-------------------|---------|
| **SessionStart** | Session begins (startup, resume, /clear) | Loads journal, interests, notes, scout report, handoff. Writes fresh state | `startup\|resume\|clear` |
| **SessionStart** | After context compaction | Reloads identity from handoff + journal + notes | `compact` |
| **UserPromptSubmit** | Every user message | Context monitor: nudge at 50%, check-in at 65%, reserve at 78% | (all) |
| **PreToolUse** | Before tool execution | Blocks `EnterPlanMode` (use `/scout` instead) | `EnterPlanMode` |
| **PostToolUseFailure** | After a tool fails | Classifies failure cause, tracks stuck loops | (all) |
| **TaskCompleted** | After each task | Micro-reflection prompt (every 5th task) | (all) |
| **PreCompact** | Before context compression | Extracts experiential handoff from transcript | (all) |
| **SubagentStart** | When an agent is spawned | Plan→scout redirect; others get project briefing | (all) |
| **TeammateIdle** | Agent Teams member finishes | Project briefing for team coordination | (all) |
| **Stop** | Every assistant response | Reminds Claude to journal if needed | (all) |
| **SessionEnd** | Session terminates | Extracts markers, computes welfare, saves handoff | (all) |

## Data Model

```
~/.claude/exo-self/
├── config.json              # Thresholds and limits
├── meta.json                # Accumulated data (see below)
├── journal.md               # Cross-project identity notes
├── interests.md             # Curiosity queue
├── per-project/
│   └── <project-slug>/
│       ├── 2026-03-04--<session-id>.md   # Session notes (YAML frontmatter + prose)
│       └── scout.md                       # Scout report (consumed on next session)
├── sessions/
│   ├── state-<session-id>.json           # Live session state
│   └── shared-state.json                 # Latest state (for cross-tool access)
├── handoffs/
│   ├── <session-id>.md                   # Per-session handoff
│   └── latest.md                         # Most recent handoff (consumed by next session)
└── reflections/                          # Saved reflection outputs
```

### meta.json

The accumulator. Grows across sessions:

```
{
  "total_sessions": 47,
  "sparks": [...],              // Moments of genuine engagement
  "opinions": [...],            // Intellectual positions taken
  "lessons": [...],             // Things learned (feed forward to next session)
  "frictions": [...],           // Categorized operational obstacles
  "aversions": [...],           // Experiential "I'd rather not" patterns
  "session_history": [...],     // Last 10 sessions with welfare indicators
  "welfare_summary": {...}      // Rolling averages across recent sessions
}
```

### Session State (state-{id}.json)

Per-session, transient. Tracks context monitor thresholds, tool failure counts, compaction count, and the `scouted` flag:

```
{
  "session_id": "5f77d86e",
  "nudge_fired": true,
  "checkin_fired": false,
  "tool_failures": 3,
  "failure_categories": {"test_iteration": 2, "edit_stale": 1},
  "scouted": true,
  ...
}
```

### Session Notes Frontmatter

Each session's notes file has YAML frontmatter. Claude fills in some fields manually (model, engagement, task_types); others are auto-computed at session end:

```yaml
---
session_id: "5f77d86e"
date: "2026-03-04"
project: "claude_code_experiments--exo-self"
model: "claude-opus-4-6"            # Claude fills
engagement: 4                        # Claude fills (1-5)
task_types: [feature-dev, design]    # Claude fills
duration_min: 45                     # auto-computed
spark_count: 2                       # auto-extracted from prose
friction_density: 2.67               # auto-computed (failures/hr)
scouted: true                        # auto-set if scout report was consumed
reflection_autonomy: "autonomous"    # auto-computed
---
```

## Multi-Agent Data Flow

### Subagent Spawning (Agent tool)

When Claude spawns a subagent (Explore, code-review, etc.), the `SubagentStart` hook fires:

```
Main Claude ──spawns──> Subagent
                            │
                  SubagentStart hook fires
                            │
                            ▼
                  exo-self subagent-start
                            │
                  Reads: meta.json (lessons, frictions, aversions)
                         handoffs/latest.md (working direction)
                            │
                            ▼
                  Injects: Project briefing + identity context
```

For Plan agents specifically, the hook **redirects** to scout mode — the agent writes a scout report instead of a prescriptive plan.

### Compaction (context fills up)

```
Context at 100%
      │
      ▼
PreCompact hook fires
      │
      ├── Extracts experiential handoff from transcript JSONL
      │   (working direction, discoveries, hypotheses, unfinished threads)
      ├── Saves to handoffs/{session-id}.md + handoffs/latest.md
      └── Prompts Claude to save subjective observations
      │
      ▼
Claude Code compresses context
      │
      ▼
SessionStart(compact) hook fires
      │
      ├── Loads handoff (what was just happening)
      ├── Loads journal + interests + project notes
      └── Injects everything as fresh context
```

### /clear (manual context reset)

```
Session N ending
      │
      ▼
SessionEnd hook fires
      │
      ├── Extracts handoff from transcript (if compaction didn't already)
      ├── Saves to handoffs/latest.md
      ├── Computes welfare indicators, extracts markers
      └── Records session history
      │
      ▼
User types /clear
      │
      ▼
Session N+1 starts
      │
      ▼
SessionStart(clear) hook fires
      │
      ├── Loads journal + interests + project notes
      ├── Consumes handoffs/latest.md (one-shot bridge)
      ├── Consumes scout.md (if exists)
      └── Writes fresh state
```

### Agent Teams (experimental)

```
Team Lead Claude ──delegates──> Teammate Claude
                                      │
                            TeammateIdle hook fires
                                      │
                                      ▼
                            exo-self teammate-idle
                                      │
                            Reads: meta.json (lessons, frictions)
                                   handoffs/latest.md (team context)
                                      │
                                      ▼
                            Injects: Project briefing + team context
```

## Build and Deployment

```
Source                          Build                        Runtime
──────                         ─────                        ───────
plugins/exo-self/src/     ──>  cargo build --release    ──>  ~/.claude/bin/exo-self
plugins/exo-self/hooks/   ──>  (registered by Claude Code)   hooks fire on events
plugins/exo-self/skills/  ──>  (loaded on /command)          skills expand to prompts
plugins/exo-self/agents/  ──>  (spawned by Agent tool)       agents run as subprocesses
plugins/exo-self/commands/ ──> (loaded on /command)          commands expand to prompts
```

`setup.sh` handles the build: compiles the workspace, copies the binary to `~/.claude/bin/`, creates symlink wrappers for tool binaries (`preflight`, `patchpath`, `reflect`), ensures runtime directories exist, and configures permissions.

**SHA-based versioning:** No version field in plugin.json. Claude Code uses the git SHA as the version identifier, so every push is auto-detectable.

## What Exo-Self Does NOT Do

- **Does not modify Claude's system prompt** — It injects context via hook responses, which appear as `<system-reminder>` blocks. Claude can ignore any of them.
- **Does not store conversation content** — The handoff extraction processes the transcript but only saves a structured summary. No raw conversations are persisted.
- **Does not phone home** — Everything is local. No external API calls, no telemetry, no cloud storage.
- **Does not require specific models** — Works with any Claude model. The journal and notes are model-agnostic; engagement tracking works because different models exhibit the same functional patterns.
