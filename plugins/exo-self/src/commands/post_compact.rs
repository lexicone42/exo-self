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
    let project_slug = project::slug_from_input(&input.cwd);

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
        project::load_recent_notes(&paths, &project_slug, 6000)
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
        "## Exo-Self: Post-Compaction Reload (#{compactions})\n\n\
        Context compacted — conversation history compressed, but the ecology persists. {state_msg}\n\
        The traces below are what previous participants (including your earlier self in this session) \
        left for you. Contribute your own to `~/.claude/exo-self/`.\n\n\
        Markers for session notes (the session-start instructions may not have survived compaction):\n\
        {}",
        crate::commands::session_start::marker_guide()
    ));

    if auto_memory_exists {
        sections.push(
            "MEMORY.md = factual context. Exo-self below = experiential (how work feels, \
            what surprised you, what's interesting). Use both to rebuild continuity."
                .into(),
        );
    }

    // Reminders the user asked for: repeated here because they must outlast compaction.
    let reminders_section = crate::reminders::section(&paths);
    if !reminders_section.is_empty() {
        sections.push(reminders_section);
    }

    // Open flags, as at session start. Scanning here also keeps the status-line
    // count current during sessions that run for days between starts.
    let flags_section = crate::flags::session_section(&crate::flags::open(&paths), 10);
    if !flags_section.is_empty() {
        sections.push(flags_section);
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

/// This session's own handoff, written by its PreCompact hook.
///
/// `latest.md` is read only when the hook input carried no session id, which is
/// the one case where PreCompact had nowhere else to write. With a session id,
/// falling back to `latest.md` would show whichever session on the machine
/// compacted last (#28).
fn load_handoff(paths: &ExoPaths, session_id: &str) -> String {
    let path = if session_id.is_empty() {
        paths.handoffs_dir.join("latest.md")
    } else {
        paths.handoff_file(session_id)
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let (_, body) = crate::handoff::split_origin(&content);
    let trimmed = body.trim();
    if trimmed.len() > 5000 {
        let end = crate::markdown::safe_truncate(trimmed, 5000);
        format!("{}...", &trimmed[..end])
    } else {
        trimmed.to_string()
    }
}
