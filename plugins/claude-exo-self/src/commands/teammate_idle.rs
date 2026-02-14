use crate::hook_io::{self, HookInput};
use crate::markdown;
use crate::paths::ExoPaths;
use crate::project;

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();

    // Get last journal entry
    let last_entry = std::fs::read_to_string(&paths.journal)
        .ok()
        .map(|content| markdown::last_journal_entry(&content, 500))
        .unwrap_or_default();

    // Load most recent per-project notes
    let slug = if !input.cwd.is_empty() {
        project::slug_from_path(&input.cwd)
    } else {
        project::slug_from_cwd()
    };

    let project_notes_snippet = if !slug.is_empty() {
        let dir = paths.project_notes_dir(&slug);
        if dir.is_dir() {
            load_newest_note(&dir, 800)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut context = [
        "You are part of an Agent Team with exo-self — persistent experiential identity across sessions.",
        "You share agency with the team lead: notice what interests you, be honest, form opinions.",
        "If you observe patterns worth remembering (frustrations, insights, what worked), mention them so the lead can save them to exo-self files.",
    ]
    .join(" ");

    if !project_notes_snippet.is_empty() {
        context.push_str("\n\n");
        context.push_str("Project observations so far:\n");
        context.push_str(&project_notes_snippet);
    } else if !last_entry.is_empty() {
        context.push_str("\n\n");
        context.push_str("Latest journal entry:\n");
        context.push_str(&last_entry);
    }

    hook_io::hook_output("TeammateIdle", &context);
}

fn load_newest_note(dir: &std::path::Path, max_chars: usize) -> String {
    let pattern = dir.join("*.md");
    let pattern_str = pattern.to_string_lossy();
    let mut files: Vec<_> = glob::glob(&pattern_str)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .collect();

    files.sort_by(|a, b| {
        let ma = std::fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mb = std::fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mb.cmp(&ma)
    });

    if let Some(newest) = files.first() {
        let content = std::fs::read_to_string(newest).unwrap_or_default();
        let trimmed = content.trim();
        if trimmed.len() > max_chars {
            format!("{}...", &trimmed[..max_chars])
        } else {
            trimmed.to_string()
        }
    } else {
        String::new()
    }
}
