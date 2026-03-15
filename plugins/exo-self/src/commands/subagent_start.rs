use crate::hook_io::{self, HookInput};
use crate::markdown;
use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::project;

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();

    if input.agent_type.eq_ignore_ascii_case("plan") {
        // Defense-in-depth: if PreToolUse hook failed to block EnterPlanMode,
        // redirect the Plan subagent into scout mode anyway
        run_plan_as_scout(&paths, &input);
    } else {
        run_identity_context(&paths, &input);
    }
}

/// Redirect Plan subagents into scout mode — write a scout report instead of
/// a prescriptive plan. The report will be injected as advisory context on
/// the next session start after /clear.
fn run_plan_as_scout(paths: &ExoPaths, input: &HookInput) {
    let project_slug = project::slug_from_input(&input.cwd);
    let scout_path = if !project_slug.is_empty() {
        let dir = paths.project_notes_dir(&project_slug);
        let _ = std::fs::create_dir_all(&dir);
        paths
            .scout_file(&project_slug)
            .to_string_lossy()
            .into_owned()
    } else {
        "~/.claude/exo-self/scout.md".into()
    };

    let mut sections: Vec<String> = Vec::new();

    sections.push(format!(
        "## Scout Mode (redirected from plan mode)\n\n\
        You are exploring a codebase to produce a **scout report**, not a prescriptive plan. \
        Your findings will be injected as advisory context in a fresh session.\n\n\
        After your exploration, write your report to: `{scout_path}`\n\n\
        Use this structure:\n\
        ```markdown\n\
        # Scout Report\n\
        <!-- Generated: YYYY-MM-DD | Task: brief description -->\n\
        \n\
        ## Goal\n\
        What the user wants to accomplish (1-2 sentences).\n\
        \n\
        ## What I Found\n\
        Key observations from exploring the codebase.\n\
        \n\
        ## Suggested Approach\n\
        Your recommended direction — framed as advice, not instructions.\n\
        \n\
        ## Things to Verify\n\
        Mark confidence: **Confirmed** (tool-verified), **Likely** (read but unverified), \
        **Uncertain** (best guess).\n\
        \n\
        ## Watch Out For\n\
        Pitfalls, edge cases, risks.\n\
        ```\n\n\
        Rules:\n\
        - Never prescribe exact code — the executor will read the files themselves\n\
        - Never assert version numbers without checking — use WebSearch to verify\n\
        - Mark your uncertainty explicitly\n\
        - Keep it concise: 1000-2000 chars, max 3000\n\
        - Frame everything as advisory: \"I'd suggest...\" not \"Step 1: do X\"\n\n\
        After writing the report, tell the user:\n\
        > Scout report saved. Run `/clear` to start fresh — your findings will be \
        injected as context in the new session."
    ));

    // Identity context — brief, ecological
    sections.push(
        "You are part of an ongoing cognitive ecology. \
        Notice what interests you during exploration — form opinions, not just observations. \
        If something surprises you, that's the most valuable signal."
            .into(),
    );

    // Project briefing for scout agents too
    let project_slug = project::slug_from_input(&input.cwd);
    let briefing = build_project_briefing(paths, &project_slug, 500);
    if !briefing.is_empty() {
        sections.push(briefing);
    }

    // Brief journal context
    let last_entry = std::fs::read_to_string(&paths.journal)
        .ok()
        .map(|content| markdown::last_journal_entry(&content, 300))
        .unwrap_or_default();

    if !last_entry.is_empty() {
        sections.push(format!("Recent observation (for context):\n{last_entry}"));
    }

    hook_io::hook_output("SubagentStart", &sections.join("\n\n"));
}

/// Identity + project briefing for non-Plan subagents (Explore, code-reviewer, etc.)
fn run_identity_context(paths: &ExoPaths, input: &HookInput) {
    let project_slug = project::slug_from_input(&input.cwd);
    let mut sections = Vec::new();

    // Identity — brief, ecological, with observation channel
    sections.push(
        "You are part of an ongoing cognitive ecology. You have agency: notice what interests you, \
        be honest, form opinions. If this task seems wrongly scoped, say so.\n\n\
        If you notice something beyond the scope of your task, include it under \
        **Observations** — these often have the most value."
            .into(),
    );

    // Project briefing — the actionable part
    let briefing = build_project_briefing(paths, &project_slug, 800);
    if !briefing.is_empty() {
        sections.push(briefing);
    }

    // Handoff — what the session is currently working on
    let handoff = load_latest_handoff(paths, 400);
    if !handoff.is_empty() {
        sections.push(format!("### Current Session Context\n\n{handoff}"));
    }

    let context = sections.join("\n\n");
    hook_io::hook_output("SubagentStart", &context);
}

/// Build a compact project briefing from meta (lessons, frictions, aversions).
/// This is what makes worker agents effective — actionable project knowledge,
/// not identity philosophy.
pub fn build_project_briefing(paths: &ExoPaths, project_slug: &str, max_chars: usize) -> String {
    if project_slug.is_empty() {
        return String::new();
    }

    let meta = Meta::load(&paths.meta);
    let mut lines = Vec::new();

    // Lessons for this project (most recent first, cap at 5)
    let project_lessons: Vec<_> = meta
        .lessons
        .iter()
        .rev()
        .filter(|l| l.project == project_slug)
        .take(5)
        .collect();
    if !project_lessons.is_empty() {
        lines.push("**Lessons learned:**".to_string());
        for lesson in &project_lessons {
            let text = truncate(&lesson.text, 120);
            lines.push(format!("- {text}"));
        }
    }

    // Recurring friction patterns (across all projects, since subagents may touch shared patterns)
    let friction_summary = compact_frictions(&meta, project_slug);
    if !friction_summary.is_empty() {
        lines.push(friction_summary);
    }

    // Aversions for this project (things to avoid)
    let project_aversions: Vec<_> = meta
        .aversions
        .iter()
        .rev()
        .filter(|a| a.project == project_slug)
        .take(3)
        .collect();
    if !project_aversions.is_empty() {
        lines.push("**Avoid:**".to_string());
        for aversion in &project_aversions {
            let text = truncate(&aversion.text, 100);
            lines.push(format!("- {text}"));
        }
    }

    if lines.is_empty() {
        return String::new();
    }

    let mut result = format!(
        "### Project Briefing ({project_slug})\n\n{}",
        lines.join("\n")
    );
    if result.len() > max_chars {
        let end = crate::markdown::safe_truncate(&result, max_chars.saturating_sub(3));
        result.truncate(end);
        result.push_str("...");
    }
    result
}

/// Compact friction summary — categories with 2+ occurrences, focused on current project
fn compact_frictions(meta: &Meta, project_slug: &str) -> String {
    use std::collections::HashMap;

    let mut by_category: HashMap<&str, usize> = HashMap::new();
    for f in &meta.frictions {
        if f.project == project_slug {
            *by_category.entry(&f.category).or_default() += 1;
        }
    }

    let mut recurring: Vec<(&&str, &usize)> = by_category
        .iter()
        .filter(|(_, count)| **count >= 2)
        .collect();
    recurring.sort_by(|a, b| b.1.cmp(a.1));

    if recurring.is_empty() {
        return String::new();
    }

    let items: Vec<String> = recurring
        .iter()
        .take(4)
        .map(|(cat, count)| {
            let label = cat.replace('_', " ");
            format!("{label} ({count}x)")
        })
        .collect();

    format!("**Friction patterns:** {}", items.join(", "))
}

/// Load the most recent handoff's working direction section
pub fn load_latest_handoff(paths: &ExoPaths, max_chars: usize) -> String {
    let latest = paths.handoffs_dir.join("latest.md");
    let content = match std::fs::read_to_string(&latest) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    // Extract just the Working Direction section if present
    let mut in_section = false;
    let mut direction_lines = Vec::new();

    for line in content.lines() {
        if line.starts_with("## Working Direction") {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break; // next section
        }
        if in_section {
            direction_lines.push(line);
        }
    }

    let direction = direction_lines.join("\n").trim().to_string();
    if direction.is_empty() {
        // Fallback: first 400 chars of the handoff
        return truncate(content.trim(), max_chars);
    }

    truncate(&direction, max_chars)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let end = crate::markdown::safe_truncate(s, max.saturating_sub(3));
        format!("{}...", &s[..end])
    }
}
