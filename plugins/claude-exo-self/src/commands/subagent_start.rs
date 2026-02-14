use crate::hook_io;
use crate::markdown;
use crate::paths::ExoPaths;

pub fn run() {
    let paths = ExoPaths::new();

    // Get last journal entry — keep it small for subagent context
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
