use crate::hook_io::{self, HookInput};
use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::project;
use crate::state::{self, SessionState};

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();

    let session_id = &input.session_id;
    let mut state = SessionState::load(&paths, session_id);
    let session_start = state.session_start;

    // --- Bookkeeping BEFORE early-exit guards ---
    let wrote_notes = project::detect_wrote_notes(&state, &paths, session_start);

    // Update checkin_responded BEFORE guards
    if wrote_notes && state.checkin_fired && !state.checkin_responded {
        state.checkin_responded = true;
        state.save(&paths);
    }

    // --- Early-exit guards ---
    if input.stop_hook_active {
        hook_io::empty_output();
        return;
    }
    if state.stop_reminded {
        hook_io::empty_output();
        return;
    }
    // Cooldown: don't block again within 60s
    if state.last_stop_time > 0.0 && (state::now() - state.last_stop_time) < 60.0 {
        hook_io::empty_output();
        return;
    }

    // --- Gather cross-signal data ---
    let duration_min = if session_start > 0.0 {
        (state::now() - session_start) / 60.0
    } else {
        0.0
    };

    // Update meta with session end time
    let mut meta = Meta::load(&paths.meta);
    meta.last_session_end = Some(
        chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%.6f")
            .to_string(),
    );
    meta.save(&paths.meta);

    // --- Decision logic ---
    let has_signal = state.checkin_fired
        || state.tool_failures >= 3
        || state.task_completions >= 2
        || state.compactions > 0;

    let should_block = !wrote_notes && duration_min >= 5.0 && has_signal;

    if should_block {
        state.stop_reminded = true;
        state.last_stop_time = state::now();

        let target = if !state.project_slug.is_empty() {
            format!("~/.claude/exo-self/per-project/{}/", state.project_slug)
        } else {
            "journal.md".into()
        };

        let mut reason = format!("Exo-self: ~{} min session", duration_min as u32);

        if state.tool_failures >= 3 {
            let top_tool = state
                .failure_tools
                .iter()
                .max_by_key(|(_, c)| *c)
                .map(|(t, _)| t.as_str())
                .unwrap_or("tools");
            reason.push_str(&format!(
                ", {} failures ({})",
                state.tool_failures, top_tool
            ));
        }
        if state.task_completions >= 2 {
            reason.push_str(&format!(", {} tasks done", state.task_completions));
        }
        if state.compactions > 0 {
            reason.push_str(&format!(", {}x compacted", state.compactions));
        }
        reason.push_str(&format!(
            ". A sentence to ~/{target}? If nothing to note, just stop."
        ));

        state.save(&paths);
        hook_io::decision_output("block", &reason);
    } else {
        state.save(&paths);
        hook_io::empty_output();
    }
}
