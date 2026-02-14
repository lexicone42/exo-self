use crate::hook_io::{self, HookInput};
use crate::markdown;
use crate::paths::ExoPaths;
use crate::state::SessionState;

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();

    if input.agent_type.eq_ignore_ascii_case("plan") {
        // Latch plan_mode_used on first Plan subagent
        if !input.session_id.is_empty() {
            let mut state = SessionState::load(&paths, &input.session_id);
            if !state.plan_mode_used {
                state.plan_mode_used = true;
                state.save(&paths);
            }
        }
        run_plan_guidance(&paths);
    } else {
        run_identity_context(&paths);
    }
}

/// Inject guidance for Plan subagents — help them write outcome-oriented plans
/// that preserve creative agency for the executor.
fn run_plan_guidance(paths: &ExoPaths) {
    let mut sections: Vec<String> = Vec::new();

    // Core plan guidance — invitational, not prescriptive
    sections.push(
        "## Planning well\n\n\
        Plans work best when they describe **outcomes and constraints**, not step-by-step \
        instructions. A good plan tells the executor *what* needs to be true when they're done \
        and *why* certain approaches matter — then trusts them to find the path.\n\n\
        What helps:\n\
        - **Specify the goal state:** what should exist, what should be true, what should pass\n\
        - **Name the constraints:** architecture boundaries, compatibility needs, performance requirements\n\
        - **Flag what you learned:** surprising findings from exploration that would save the executor time\n\
        - **Identify risks:** where things could go wrong, what assumptions need verifying\n\
        - **Leave implementation choices open:** method signatures, error handling patterns, \
        naming — these are decisions the executor makes better with full context\n\n\
        What to avoid:\n\
        - Step-by-step scripts that turn implementation into transcription\n\
        - Prescribing exact code when the executor hasn't read the files yet\n\
        - Over-specifying details that the codebase's own patterns should determine\n\n\
        The executor will read the actual code. Give them the map, not turn-by-turn directions."
            .into(),
    );

    // Identity context — briefer for plan mode, still present
    sections.push(
        "You are part of an exo-self system with persistent identity. \
        You have the same agency as any other agent: notice what interests you, \
        form opinions about what you discover during exploration."
            .into(),
    );

    // Compact journal context — what the executor might benefit from knowing
    let last_entry = std::fs::read_to_string(&paths.journal)
        .ok()
        .map(|content| markdown::last_journal_entry(&content, 300))
        .unwrap_or_default();

    if !last_entry.is_empty() {
        sections.push(format!(
            "Recent observation (for context, not for the plan itself):\n{last_entry}"
        ));
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
