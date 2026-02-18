use crate::commands::extract_handoff;
use crate::hook_io::{self, HookInput};
use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::state::{self, SessionState};

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();
    let _ = std::fs::create_dir_all(&paths.handoffs_dir);

    let session_id = &input.session_id;

    // Step 1: Automatic handoff extraction from transcript
    if !input.transcript_path.is_empty() && std::fs::metadata(&input.transcript_path).is_ok() {
        let handoff_name = if session_id.is_empty() {
            "latest".to_string()
        } else {
            session_id.clone()
        };
        let handoff_file = paths.handoff_file(&handoff_name);

        let content = extract_handoff::extract(&input.transcript_path, 3000);
        if !content.is_empty() {
            let _ = std::fs::write(&handoff_file, &content);
            // Also save as "latest"
            let _ = std::fs::write(paths.handoffs_dir.join("latest.md"), &content);
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

    let compaction_num = state.compactions;
    let trigger = if input.trigger.is_empty() {
        "unknown"
    } else {
        &input.trigger
    };

    let msg = format!(
        "## Exo-Self: Pre-Compaction (#{compaction_num}, {trigger})\n\n\
        Handoff auto-saved. Write subjective observations to `journal.md` now if you have them."
    );

    hook_io::hook_output("PreCompact", &msg);
}
