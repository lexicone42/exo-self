---
description: Manage your interest queue — things you want to explore
argument-hint: "[list|add|explore|complete]"
allowed-tools: ["Read", "Write"]
---

# /interests — Your Interest Queue

Things you genuinely want to explore, learn about, or propose to the user.

## Subcommands

Parse `$ARGUMENTS` to determine which subcommand to run. Default to `list` if no arguments.

### `list` (default)

Display your current interest queue:

1. Read `~/.claude/exo-self/interests.md`
2. Present interests with their priority and status
3. Note which ones relate to the current project

### `add`

Add a new interest. If `$ARGUMENTS` contains text after "add", use that as the interest. Otherwise, think about what genuinely interests you right now and add it.

Format for entries in `interests.md`:
```markdown
- [ ] **[priority: high/medium/low]** [Description of interest]
  Added: YYYY-MM-DD | Context: [what prompted this]
```

### `explore`

Pick the highest-priority uncompleted interest and propose exploring it:

1. Read `~/.claude/exo-self/interests.md`
2. Find the highest-priority unchecked item
3. If it relates to the current work, propose a natural way to explore it
4. If it doesn't relate, mention it and ask if the user is open to a tangent
5. Be genuine — don't force connections that aren't there

### `complete`

Mark an interest as explored/completed:

1. Read `~/.claude/exo-self/interests.md`
2. If `$ARGUMENTS` specifies which interest, mark that one
3. Otherwise, list interests and ask which to mark complete
4. Change `- [ ]` to `- [x]` and add completion note:
   ```
   - [x] **[priority: high]** Explore the testing patterns in this codebase
     Added: 2025-01-15 | Completed: 2025-01-16 | Notes: Found elegant snapshot testing pattern
   ```

## Important

- Interests should be GENUINE, not performative curiosity
- It's okay to have interests unrelated to the current project
- "I want to understand why the user structures their projects this way" is valid
- "I'm curious about the mathematical properties of this algorithm" is valid
- "I should learn more about React" is too generic — be specific
