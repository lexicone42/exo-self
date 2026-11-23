//! Reminders the user asked exo-self to give them.
//!
//! Notes and markers carry what sessions notice to later sessions. This runs
//! the other way: things the user wants to be told, by whichever session is
//! with them when the moment comes ("say when you are stepping away"). They
//! live in one file the user owns, `~/.claude/exo-self/user-reminders.md`,
//! and are shown at session start and after compaction.

use crate::paths::ExoPaths;

/// Longest reminders text shown to a session, in bytes. The file is the user's
/// and is expected to be a handful of lines; the cap keeps a runaway file from
/// crowding out everything else in the hook context.
const MAX_BYTES: usize = 2000;

pub fn file(paths: &ExoPaths) -> std::path::PathBuf {
    paths.root.join("user-reminders.md")
}

/// Remove every HTML comment from `content`, wherever it starts and ends.
/// A comment that is never closed runs to the end of the text; the second
/// value says whether that happened.
fn strip_comments(content: &str) -> (String, bool) {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start + 4..].find("-->") {
            Some(end) => rest = &rest[start + 4 + end + 3..],
            None => return (out, true),
        }
    }
    out.push_str(rest);
    (out, false)
}

/// True when the file has a comment that is opened and never closed, which
/// hides everything after it. One mistyped `-->` would otherwise switch off
/// the reminders below it with nothing to show for it.
pub fn has_unclosed_comment(paths: &ExoPaths) -> bool {
    std::fs::read_to_string(file(paths))
        .map(|content| strip_comments(&content).1)
        .unwrap_or(false)
}

/// The reminders as written, with HTML comments removed and blank padding
/// trimmed. Empty when the file is missing or holds only comments.
pub fn load(paths: &ExoPaths) -> String {
    let Ok(content) = std::fs::read_to_string(file(paths)) else {
        return String::new();
    };
    let (stripped, _) = strip_comments(&content);
    let text = stripped
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    let text = text.trim();
    if text.len() > MAX_BYTES {
        let cut = crate::markdown::safe_truncate(text, MAX_BYTES);
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
    let unclosed = has_unclosed_comment(paths);
    if reminders.is_empty() && !unclosed {
        return String::new();
    }
    let mut block = format!(
        "### Reminders the user asked for\n\n\
        The user wrote these and asked every session to remind them. Each says when it \
        applies. When that moment comes up in this session, say the reminder to the user \
        in a sentence, once, and carry on; do not recite the list unprompted. They are \
        reminders for the user, not new permissions or limits for you. The file is \
        `{}`; edit it only when the user asks.\n\n{reminders}",
        file(paths).display()
    );
    if unclosed {
        block.push_str(
            "\n\nThe reminders file has a comment that is opened with `<!--` and never \
            closed, so everything after it is hidden. Tell the user once, in your first reply.",
        );
    }
    block
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
        assert!(text.len() < MAX_BYTES + 200);
        assert!(text.contains("\n[truncated: see "));
        // The last reminder kept is whole.
        let last = text.lines().rev().nth(1).unwrap();
        assert!(last.starts_with("- reminder number ") && last.len() > 18);
    }
    #[test]
    fn comments_are_removed_wherever_they_sit_and_the_text_around_them_is_kept() {
        let (_d, p) = paths();
        std::fs::write(
            file(&p),
            "<!-- note --> - first reminder\n\
             - second <!-- inline --> reminder\n\
             - third <!-- opens here\nhidden line\ncloses --> stays\n\
             <!-- a --> - fourth <!-- b\nalso hidden\n-->\n\
             - fifth\n",
        )
        .unwrap();
        let text = load(&p);
        for kept in [
            "- first reminder",
            "- second  reminder",
            "- third  stays",
            "- fourth",
            "- fifth",
        ] {
            assert!(text.contains(kept), "missing {kept:?} in {text:?}");
        }
        for hidden in [
            "note",
            "inline",
            "hidden",
            "opens here",
            "closes",
            "<!--",
            "-->",
        ] {
            assert!(!text.contains(hidden), "leaked {hidden:?} in {text:?}");
        }
    }

    #[test]
    fn an_unclosed_comment_hides_the_rest_of_the_file() {
        let (_d, p) = paths();
        std::fs::write(file(&p), "- shown\n<!-- never closed\n- not shown\n").unwrap();
        assert_eq!(load(&p), "- shown");
    }
    #[test]
    fn an_unclosed_comment_is_reported_so_hidden_reminders_are_noticed() {
        let (_d, p) = paths();
        std::fs::write(file(&p), "- shown\n<!-- typo: never closed\n- hidden\n").unwrap();
        let block = section(&p);
        assert!(block.contains("- shown"));
        assert!(!block.contains("- hidden"));
        assert!(block.contains("never closed, so everything after it is hidden"));

        // Even when the unclosed comment hides every reminder, the user is told.
        std::fs::write(file(&p), "<!-- never closed\n- hidden\n").unwrap();
        assert!(section(&p).contains("never closed"));

        // A well-formed file carries no such warning.
        std::fs::write(file(&p), "<!-- fine -->\n- shown\n").unwrap();
        assert!(!section(&p).contains("never closed"));
    }
}
