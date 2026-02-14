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

    // Track task completions
    state.task_completions += 1;
    state.last_task_completed_at = state::now();

    // Nudge on every 5th task completion
    if state.task_completions.is_multiple_of(5) && !state.task_reflection_suppressed {
        let msg = format!(
            "Exo-self: {} tasks done. Anything worth noting in your session notes?",
            state.task_completions
        );
        state.save(&paths);
        hook_io::hook_output("TaskCompleted", &msg);
    } else {
        state.save(&paths);
        hook_io::empty_output();
    }
}
