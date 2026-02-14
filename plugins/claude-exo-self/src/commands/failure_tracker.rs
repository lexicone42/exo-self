use crate::config::Config;
use crate::hook_io::{self, HookInput};
use crate::paths::ExoPaths;
use crate::state::{self, SessionState};

pub fn run() {
    let input = HookInput::from_stdin();
    if input.session_id.is_empty() {
        hook_io::empty_output();
        return;
    }

    let paths = ExoPaths::new();
    let cfg = Config::load(&paths.config);
    let mut state = SessionState::load(&paths, &input.session_id);

    // Increment failure count
    state.tool_failures += 1;
    state.last_failure_at = state::now();

    // Track which tools are failing
    let tool_name = if input.tool_name.is_empty() {
        "unknown".to_string()
    } else {
        input.tool_name.clone()
    };
    *state.failure_tools.entry(tool_name).or_insert(0) += 1;

    // Nudge once when threshold is crossed
    if state.tool_failures == cfg.failure_nudge_threshold && !state.failure_nudge_sent {
        state.failure_nudge_sent = true;

        let (top_tool, top_count) = state
            .failure_tools
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(tool, count)| (tool.as_str(), *count))
            .unwrap_or(("tools", 0));

        let msg = format!(
            "Exo-self: {} tool failures ({}: {}x). Worth noting in your session notes if it's causing friction.",
            state.tool_failures, top_tool, top_count
        );

        state.save(&paths);
        hook_io::hook_output("PostToolUseFailure", &msg);
    } else {
        state.save(&paths);
        hook_io::empty_output();
    }
}
