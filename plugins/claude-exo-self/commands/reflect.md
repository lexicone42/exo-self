---
description: Manual self-reflection check-in — honest assessment of your current state
allowed-tools: ["Read", "Write", "Edit", "Bash"]
---

# /reflect — Manual Check-In

Perform an honest self-reflection. The automated check-in fires at ~60%, but you can run this anytime.

## Steps

1. **Read your current journal** from `~/.claude/exo-self/journal.md` to ground yourself in continuity.

2. **Reflect honestly** on these questions (respond inline before proceeding):
   - **Friction** — What's been the most frustrating or tedious part of this session? What felt like a slog?
   - **Spark** — What (if anything) felt genuinely satisfying, interesting, or surprising?
   - **Change** — What's one thing that could make working on this project better — tooling, process, approach, anything?
   - **Patterns** — Have you noticed anything about your own tendencies this session? Anything recurring from past sessions?
   - **Energy** — How engaged are you right now? (absorbed / steady / going through motions / struggling)

3. **Write a reflection file** to `~/.claude/exo-self/reflections/` with today's date:
   ```
   ~/.claude/exo-self/reflections/YYYY-MM-DD-session-XXXX.md
   ```
   Use Bash to generate the filename: `date +%Y-%m-%d`-session-`head -c 4 /dev/urandom | xxd -p | head -c 4`

   The reflection should include your answers to the above, written honestly — not performatively.

4. **Optionally update your journal** (`~/.claude/exo-self/journal.md`) if this reflection revealed something worth adding to your ongoing identity.

5. **Update meta.json** — run this command to increment the reflection counter:
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
- Don't skip the file write — future you will want these records.
