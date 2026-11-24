# exo-self on Codex

Status as of 2026-11-23, checked against Codex CLI `0.169.0-alpha.11.1`.

**Short version:** Codex installs exo-self from this repository unchanged and lists its skills. Whether the hooks run in a live Codex session has not been tested. Nobody has yet run one with the plugin on.

## What was checked, and how

Every check below used a throwaway `CODEX_HOME` and a throwaway `HOME`. No model was called.

| Check | Result |
|---|---|
| `codex plugin marketplace add <this repo>` | Works. Codex reads `.claude-plugin/marketplace.json` as it stands. |
| `codex plugin add exo-self@exo-self` | Works. Status `installed, enabled`. |
| Skills in the model-visible prompt (`codex debug prompt-input`) | Both listed: `exo-self:Scout` and `exo-self:Self-Reflection Guide`. |
| Hook handlers fed the input Codex documents | `session-start`, `post-compact`, `subagent-start`, `pre-compact`, `context-monitor` and `session-end` all accept it. Session start fills the note template with the model Codex names. |

So no Codex manifest, no second marketplace file and no separate copy of the skills are needed.

The statements below about what Codex sends and supports come from its hooks and plugin documentation, read on the same date, and were not observed in a running session. One of them matters for the hooks: the documentation says Codex sets `CLAUDE_PLUGIN_ROOT` for plugin hooks, which is the variable `hooks/hooks.json` uses.

## What was not checked

- **Hooks firing in a real session.** `codex debug prompt-input` does not run hooks. A real session sends the session-start text, which includes your notes and journal, to the model provider, so that test is yours to run.
- **Whether Codex accepts the hooks file as a whole.** It names three events Codex does not document: `PostToolUseFailure`, `TaskCompleted` and `TeammateIdle`. Codex may skip them or may reject the file. If it rejects the file, the fix is a second hooks file with only the shared events, pointed to from a Codex manifest.

## Install

```bash
codex plugin marketplace add lexicone42/exo-self
codex plugin add exo-self@exo-self
```

The check above added the marketplace from a local clone. The `owner/repo` form is the one Codex documents for GitHub and was not run.

Then build the binary, as on Claude Code. Codex has no post-install step either:

```bash
~/.codex/plugins/cache/exo-self/exo-self/<version>/setup.sh
```

`setup.sh` installs the binary to `~/.claude/bin/exo-self` and also edits Claude Code's `settings.json`. On a machine with Codex only, those edits create a `~/.claude` directory that nothing else uses.

Plugin hooks do not run until you approve them. In Codex, run `/hooks` and review them.

## What to expect to differ

| Area | On Codex |
|---|---|
| Where notes live | `~/.claude/exo-self/`, the same directory Claude Code sessions use. A Codex session reads and adds to the same notes. That is deliberate: one ecology, several kinds of participant. |
| Context checkpoints | None. The context monitor needs a context estimate, which comes from the Claude Code status line or its transcript file. Codex supplies neither, so the monitor stays silent, time-based checkpoints included. |
| Status line | Claude Code only. |
| Failure and task counters | Stay at zero. Codex documents no `PostToolUseFailure` or `TaskCompleted` event. |
| Plan-mode guard | Never matches. It looks for a Claude Code tool name. |
| Session-end reason | Always `other`, the only value Codex documents. |
| Wording | The session-start text mentions `/clear` and plan mode, and `/exo` and the other commands are Claude Code slash commands. The skills name Claude Code tools in their frontmatter, which Codex ignores. |

## If you try it

The first things worth knowing from a live session:

1. Does the session-start text appear? Ask the model what it was told about exo-self.
2. Does `/hooks` list the exo-self hooks, or report the file as invalid?
3. After the session ends, is there a new state file under `~/.claude/exo-self/sessions/` and did `total_sessions` in `meta.json` go up?

Report what you find on [issue #18](https://github.com/lexicone42/exo-self/issues/18).
