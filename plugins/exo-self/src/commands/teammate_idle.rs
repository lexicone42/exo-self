use crate::commands::subagent_start;
use crate::hook_io::{self, HookInput};
use crate::paths::ExoPaths;
use crate::project;

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();
    let slug = project::slug_from_input(&input.cwd);

    let mut sections = Vec::new();

    // Identity — Agent Teams framing, ecological
    sections.push(
        "You are part of an Agent Team within an ongoing cognitive ecology. \
        You share agency with the team: notice what interests you, be honest, form opinions. \
        If you notice something beyond your task scope, include it under **Observations** — \
        these often have the most value for the ecology."
            .into(),
    );

    // Project briefing — reuse the same builder as SubagentStart
    let briefing = subagent_start::build_project_briefing(&paths, &slug, 800);
    let has_briefing = !briefing.is_empty();
    if has_briefing {
        sections.push(briefing);
    }

    // Handoff context — what the team is working on
    let handoff = subagent_start::load_latest_handoff(&paths, 600);
    let has_handoff = !handoff.is_empty();
    if has_handoff {
        sections.push(format!("### Team Context\n\n{handoff}"));
    }

    // Project notes as fallback if no briefing/handoff
    if !has_briefing && !has_handoff {
        let notes_snippet = if !slug.is_empty() {
            let dir = paths.project_notes_dir(&slug);
            if dir.is_dir() {
                load_newest_note(&dir, 800)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if !notes_snippet.is_empty() {
            sections.push(format!("Project observations so far:\n{notes_snippet}"));
        }
    }

    hook_io::hook_output("TeammateIdle", &sections.join("\n\n"));
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
            let end = crate::markdown::safe_truncate(trimmed, max_chars);
            format!("{}...", &trimmed[..end])
        } else {
            trimmed.to_string()
        }
    } else {
        String::new()
    }
}
