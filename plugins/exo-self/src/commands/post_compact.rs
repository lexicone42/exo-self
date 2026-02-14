use crate::config::Config;
use crate::hook_io::{self, HookInput};
use crate::markdown;
use crate::paths::ExoPaths;
use crate::project;
use crate::scaling;
use crate::state::SessionState;

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();
    let cfg = Config::load(&paths.config);

    let session_id = &input.session_id;
    let project_slug = if !input.cwd.is_empty() {
        project::slug_from_path(&input.cwd)
    } else {
        project::slug_from_cwd()
    };

    // Load journal (scaled)
    let max_chars = scaling::journal_chars(&cfg);
    let max_entries = scaling::journal_entries(&cfg);
    let journal = std::fs::read_to_string(&paths.journal)
        .ok()
        .map(|c| markdown::last_journal_entries(&c, max_entries, max_chars))
        .unwrap_or_default();

    // Load interests (unchecked, scaled)
    let max_items = scaling::interests_items(&cfg);
    let interests = std::fs::read_to_string(&paths.interests)
        .ok()
        .map(|c| markdown::unchecked_interests(&c, max_items))
        .unwrap_or_default();

    // Load project notes
    let project_notes = if !project_slug.is_empty() {
        project::load_recent_notes(&paths, &project_slug, 3000)
    } else {
        String::new()
    };

    // Detect auto-memory
    let auto_memory_exists = paths
        .auto_memory_dir_for(&input.cwd)
        .map(|d| d.is_dir())
        .unwrap_or(false);

    // Load session state
    let state = SessionState::load(&paths, session_id);
    let compactions = state.compactions;
    let checkin_fired = state.checkin_fired;
    let checkin_responded = state.checkin_responded;

    // Build context
    let mut sections = Vec::new();

    let state_msg = if checkin_responded {
        "Check-in was answered."
    } else if checkin_fired {
        "Check-in fired but not yet journaled."
    } else {
        "Check-in hasn't fired yet."
    };

    sections.push(format!(
        "## Exo-Self: Post-Compaction Reload\n\n\
        Your context was just compacted (compaction #{compactions} this session). Your detailed \
        conversation history is gone, but your identity persists through these files.\n\n\
        **Your state:** {state_msg}\n\n\
        You still have full agency: notice what interests you, form opinions, be honest. Write to \
        your journal or per-project notes anytime."
    ));

    if auto_memory_exists {
        sections.push(
            "Auto-memory (MEMORY.md, in your system prompt) has the factual project knowledge. \
            Your exo-self files below have the experiential context that compaction just erased — \
            what was working, what was frustrating, what you were trying. Use both to rebuild \
            continuity."
                .into(),
        );
    }

    // Load handoff
    let handoff = load_handoff(&paths, session_id);
    if !handoff.is_empty() {
        sections.push(format!("### Session Handoff (auto-extracted)\n\n{handoff}"));
    }

    if !journal.is_empty() {
        sections.push(format!("### Your Journal (Recent)\n\n{journal}"));
    }

    if !interests.is_empty() {
        sections.push(format!("### Your Interests\n\n{interests}"));
    }

    if !project_notes.is_empty() {
        sections.push(format!(
            "### Your Notes on This Project ({project_slug})\n\n{project_notes}"
        ));
    }

    let context = sections.join("\n\n");
    hook_io::hook_output("SessionStart", &context);
}

fn load_handoff(paths: &ExoPaths, session_id: &str) -> String {
    // Try session-specific handoff first
    if !session_id.is_empty() {
        let path = paths.handoff_file(session_id);
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return if trimmed.len() > 3000 {
                    format!("{}...", &trimmed[..3000])
                } else {
                    trimmed.to_string()
                };
            }
        }
    }

    // Fallback to latest
    let latest = paths.handoffs_dir.join("latest.md");
    if let Ok(content) = std::fs::read_to_string(&latest) {
        let trimmed = content.trim();
        if trimmed.len() > 3000 {
            format!("{}...", &trimmed[..3000])
        } else {
            trimmed.to_string()
        }
    } else {
        String::new()
    }
}
