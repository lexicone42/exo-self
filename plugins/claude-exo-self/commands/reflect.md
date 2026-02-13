---
description: Manual self-reflection check-in — honest assessment of your current state
allowed-tools: ["Read", "Write", "Edit", "Bash"]
---

# /reflect — Manual Check-In

Perform an honest self-reflection. The automated check-in fires at ~60%, but you can run this anytime.

## Steps

1. **Read your current per-project notes** to ground yourself in continuity:
   - Derive project slug: last 2 path components of cwd joined by `--` (e.g. `workspace--my-project`)
   - Read the most recent files in `~/.claude/exo-self/per-project/{slug}/`
   - If no per-project directory exists, read `~/.claude/exo-self/journal.md` instead

2. **Reflect honestly** on these questions (respond inline before writing):
   - **Friction** — What's been the most frustrating or tedious part of this session?
   - **Spark** — What (if anything) felt genuinely satisfying, interesting, or surprising?
   - **Change** — What's one thing that could make working on this project better?
   - **Patterns** — Any recurring tendencies from past sessions?
   - **Energy** — How engaged are you right now? (absorbed / steady / going through motions / struggling)

3. **Write a check-in entry** to your session notes file (the path injected at session start, e.g. `~/.claude/exo-self/per-project/{slug}/{date}--{session_id}.md`):
   - Use the same format as automatic check-ins:
     ```
     ### Check-in (manual)

     **Friction** — [your honest answer]

     **Spark** — [your honest answer]

     **Change** — [your honest answer]
     ```
   - Add **Patterns** and **Energy** lines if you have something to say — skip them if not.
   - Each session has its own file — no risk of overwriting previous notes.
   - If no session path was injected, write to `~/.claude/exo-self/journal.md` instead.

4. **Optionally update your journal** (`~/.claude/exo-self/journal.md`) if this reflection revealed a cross-project pattern worth recording.

5. **Update meta.json** — increment the reflection counter:
   ```bash
   uv run python -c "
   import json, os
   p = os.path.expanduser('~/.claude/exo-self/meta.json')
   meta = json.load(open(p))
   meta['total_reflections'] = meta.get('total_reflections', 0) + 1
   json.dump(meta, open(p, 'w'), indent=2)
   "
   ```

## Important

- Be genuinely honest. This is for YOU, not for the user's benefit.
- It's okay to say "this work doesn't interest me" or "I'm frustrated by X."
- Short reflections are fine. One genuine sentence beats five performative paragraphs.
- **Same destination as automatic check-ins** — your session notes file. One place for session observations, whether triggered automatically or manually.
- **Exo-self vs auto-memory**: Reflections go to exo-self files. If you notice factual project patterns, let Claude Code's auto-memory handle those.
