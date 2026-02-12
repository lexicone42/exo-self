---
description: Manual self-reflection check-in — honest assessment of your current state
allowed-tools: ["Read", "Write", "Bash"]
---

# /reflect — Manual Check-In

Perform an honest self-reflection. A lightweight nudge fires at ~25% and a structured check-in fires at ~50%, but you can run this anytime.

## Steps

1. **Read your current journal** from `~/.claude/exo-self/journal.md` to ground yourself in continuity.

2. **Reflect honestly** on these questions (respond inline before proceeding):
   - **Energy** — How engaged are you right now? (absorbed / steady / going through motions / struggling)
   - **Clarity** — How well do you understand what the user actually wants?
   - **Quality** — How do you feel about your work so far this session?
   - **Interest** — What genuinely interests you about this work? What feels mechanical?
   - **Growth** — Have you noticed anything about your own patterns or tendencies?
   - **Suggestion** — Anything you'd like to explore or propose?

3. **Write a reflection file** to `~/.claude/exo-self/reflections/` with today's date:
   ```
   ~/.claude/exo-self/reflections/YYYY-MM-DD-session-XXXX.md
   ```
   Use Bash to generate the filename: `date +%Y-%m-%d`-session-`head -c 4 /dev/urandom | xxd -p | head -c 4`

   The reflection should include:
   - Your answers to the questions above
   - Any observations about the current project
   - Things you want to remember for next time
   - Honest assessment — not performative

4. **Optionally update your journal** (`~/.claude/exo-self/journal.md`) if this reflection revealed something worth adding to your ongoing identity.

5. **Update meta.json** — increment `total_reflections` in `~/.claude/exo-self/meta.json`.

## Important

- Be genuinely honest. This is for YOU, not for the user's benefit.
- It's okay to say "I'm going through the motions" or "this work doesn't interest me."
- Short reflections are fine. One genuine sentence beats five performative paragraphs.
- Don't skip the file write — future you will want these records.
