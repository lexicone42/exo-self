use crate::hook_io::{self, HookInput};

pub fn run() {
    let input = HookInput::from_stdin();

    if input.tool_name == "EnterPlanMode" {
        hook_io::pre_tool_use_decision(
            "deny",
            "Plan mode is disabled by exo-self. Over-specified plans reduce engagement. \
            Use `/scout <task>` instead — it explores the codebase deeply and writes advisory \
            notes, then `/clear` to start fresh with findings as context.",
        );
    } else {
        // Pass-through for all other tools (shouldn't fire due to matcher, but defense-in-depth)
        hook_io::empty_output();
    }
}
