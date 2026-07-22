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
    let mut state = SessionState::load(&paths, &input.session_id);

    // Record the completion — the counter is consumed by context_monitor's engagement
    // detection (Grinding/Struggling patterns), so this side-effect must survive.
    state.task_completions += 1;
    state.last_task_completed_at = state::now();
    state.save(&paths);

    // Current Claude Code rejects hookSpecificOutput/additionalContext on TaskCompleted
    // (#19 audit; docs confirm the event accepts only exit-code-2 / continue:false). The
    // former "every 5th task" reflection nudge can no longer be injected here — and the
    // UserPromptSubmit checkpoints already carry the reflective prompting — so this handler
    // now records the completion and returns a no-op instead of hard-failing validation.
    hook_io::empty_output();
}
