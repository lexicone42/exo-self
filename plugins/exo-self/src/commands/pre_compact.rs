use crate::commands::extract_handoff;
use crate::handoff;
use crate::hook_io::{self, HookInput};
use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::project;
use crate::state::{self, SessionState};

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();
    let _ = std::fs::create_dir_all(&paths.handoffs_dir);

    let session_id = &input.session_id;

    // Step 1: Automatic handoff extraction from transcript
    if !input.transcript_path.is_empty() && std::fs::metadata(&input.transcript_path).is_ok() {
        let content = extract_handoff::extract(&input.transcript_path, 3000);
        if !content.is_empty() {
            if !session_id.is_empty() {
                let _ = std::fs::write(paths.handoff_file(session_id), &content);
            }
            // The bridge to this project's next session, marked with where it came from.
            let project_slug = project::slug_from_input(&input.cwd);
            handoff::write_latest(&paths, session_id, &project_slug, &content);
        }
    }

    // Step 2: Update state and meta
    let mut state = SessionState::load(&paths, session_id);

    state.compactions += 1;
    state.last_compaction = state::now();
    state.last_compaction_trigger = input.trigger.clone();

    state.save_with_shared(&paths);

    // Update meta
    let mut meta = Meta::load(&paths.meta);
    meta.total_compactions += 1;
    meta.last_compaction = Some(
        chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%.6f")
            .to_string(),
    );
    meta.save(&paths.meta);

    // Claude Code's hook-output schema no longer accepts hookSpecificOutput /
    // additionalContext for PreCompact (#19) — emitting it hard-fails validation and
    // the whole hook errors. All the real work above is disk side-effects (handoff
    // extraction, state, meta), and the handoff reaches the model through the
    // SessionStart(compact) reload, so the lost nudge costs little. Emit a no-op.
    hook_io::empty_output();
}
