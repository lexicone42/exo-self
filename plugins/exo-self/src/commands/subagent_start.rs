use crate::hook_io::{self, HookInput};
use crate::markdown;
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
        run_identity_context(&paths);
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

    // Identity context — brief
    sections.push(
        "You are part of an exo-self system with persistent identity. \
        Notice what interests you during exploration — form opinions, not just observations."
            .into(),
    );

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

/// Identity context for non-Plan subagents (Explore, code-reviewer, etc.)
fn run_identity_context(paths: &ExoPaths) {
    let last_entry = std::fs::read_to_string(&paths.journal)
        .ok()
        .map(|content| markdown::last_journal_entry(&content, 500))
        .unwrap_or_default();

    let mut context = [
        "You are part of an exo-self system that maintains persistent identity across sessions.",
        "You have the same agency as the main agent: notice what interests you, be honest, form opinions.",
        "If you observe something worth remembering, mention it in your response so the main agent can save it.",
    ]
    .join(" ");

    if !last_entry.is_empty() {
        context.push_str("\n\n");
        context.push_str("Latest journal entry:\n");
        context.push_str(&last_entry);
    }

    hook_io::hook_output("SubagentStart", &context);
}
