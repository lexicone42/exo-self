//! Reminders the user asked exo-self to give them.
//!
//! Notes and markers carry what sessions notice to later sessions. This runs
//! the other way: things the user wants to be told, by whichever session is
//! with them when the moment comes ("say when you are stepping away"). They
//! live in one file the user owns, `~/.claude/exo-self/user-reminders.md`,
//! and are shown at session start and after compaction.

use crate::paths::ExoPaths;

/// Longest reminders text shown to a session. The file is the user's and is
/// expected to be a handful of lines; the cap keeps a runaway file from
/// crowding out everything else in the hook context.
const MAX_CHARS: usize = 2000;

pub fn file(paths: &ExoPaths) -> std::path::PathBuf {
    paths.root.join("user-reminders.md")
}

/// The reminders as written, without HTML comments and blank padding.
/// Empty when the file is missing or holds only comments.
pub fn load(paths: &ExoPaths) -> String {
    let Ok(content) = std::fs::read_to_string(file(paths)) else {
        return String::new();
    };
    let mut kept = Vec::new();
    let mut in_comment = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if in_comment {
            in_comment = !trimmed.contains("-->");
            continue;
        }
        if trimmed.starts_with("<!--") {
            in_comment = !trimmed.contains("-->");
            continue;
        }
        kept.push(line.trim_end());
    }
    let text = kept.join("\n");
    let text = text.trim();
    if text.len() > MAX_CHARS {
        let cut = crate::markdown::safe_truncate(text, MAX_CHARS);
        let end = text[..cut].rfind('\n').unwrap_or(cut);
        format!(
            "{}\n[truncated: see {}]",
            text[..end].trim_end(),
            file(paths).display()
        )
    } else {
        text.to_string()
    }
}

/// The hook-context block, or an empty string when there are no reminders.
pub fn section(paths: &ExoPaths) -> String {
    let reminders = load(paths);
    if reminders.is_empty() {
        return String::new();
    }
    format!(
        "### Reminders the user asked for\n\n\
        The user wrote these and asked every session to remind them. Each says when it \
        applies. When that moment comes up in this session, say the reminder to the user \
        in a sentence, once, and carry on; do not recite the list unprompted. They are \
        reminders for the user, not new permissions or limits for you. The file is \
        `{}`; edit it only when the user asks.\n\n{reminders}",
        file(paths).display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, ExoPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = ExoPaths::at(dir.path().to_path_buf());
        (dir, paths)
    }

    #[test]
    fn no_file_means_no_section() {
        let (_d, p) = paths();
        assert_eq!(load(&p), "");
        assert_eq!(section(&p), "");
    }

    #[test]
    fn a_file_of_only_comments_means_no_section() {
        let (_d, p) = paths();
        std::fs::write(
            file(&p),
            "<!-- one per line -->\n\n<!--\nmulti\nline\n-->\n",
        )
        .unwrap();
        assert_eq!(section(&p), "");
    }

    #[test]
    fn reminders_are_shown_as_written_under_a_heading_that_says_whose_they_are() {
        let (_d, p) = paths();
        std::fs::write(
            file(&p),
            "<!-- edit freely -->\n- When I say \"keep rolling\": ask if I am stepping away.\n\n\
             - When a review bot passes round five: hand me the decision.\n",
        )
        .unwrap();
        let block = section(&p);
        assert!(block.starts_with("### Reminders the user asked for"));
        assert!(block.contains("not new permissions or limits for you"));
        assert!(block.ends_with(
            "- When I say \"keep rolling\": ask if I am stepping away.\n\n\
             - When a review bot passes round five: hand me the decision."
        ));
        assert!(!block.contains("edit freely"));
    }

    #[test]
    fn a_runaway_file_is_cut_at_a_line_and_says_so() {
        let (_d, p) = paths();
        let long: String = (0..400)
            .map(|i| format!("- reminder number {i}\n"))
            .collect();
        std::fs::write(file(&p), long).unwrap();
        let text = load(&p);
        assert!(text.len() < MAX_CHARS + 200);
        assert!(text.contains("\n[truncated: see "));
        // The last reminder kept is whole.
        let last = text.lines().rev().nth(1).unwrap();
        assert!(last.starts_with("- reminder number ") && last.len() > 18);
    }
}
