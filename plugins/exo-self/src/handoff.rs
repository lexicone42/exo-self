//! Handoff files: who writes them and who may read them back.
//!
//! A handoff is text one session extracted from its own transcript. It often
//! quotes what that session said to its user, so it must not reach another
//! session looking like instructions (#28). Two files exist:
//!
//!   * `handoffs/<session_id>.md` is read only by that session: after its own
//!     compaction, and by the subagents it starts.
//!   * `handoffs/latest.md` is the bridge to the *next* session in the same
//!     project. It records which session and project wrote it, so a reader can
//!     skip a handoff from elsewhere and label one it does show.

use crate::paths::ExoPaths;

const ORIGIN_PREFIX: &str = "<!-- exo-handoff ";
const ORIGIN_SUFFIX: &str = "-->";

/// Which session wrote a `latest.md`, parsed from its first line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub session: String,
    pub project: String,
    pub written: String,
}

/// Write `latest.md` with a first line naming the session and project it came from.
pub fn write_latest(paths: &ExoPaths, session_id: &str, project_slug: &str, content: &str) {
    let written = chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string();
    let header = format!(
        "{ORIGIN_PREFIX}session={} project={} written={written} {ORIGIN_SUFFIX}\n",
        header_value(session_id),
        header_value(project_slug),
    );
    let _ = std::fs::create_dir_all(&paths.handoffs_dir);
    let _ = std::fs::write(
        paths.handoffs_dir.join("latest.md"),
        format!("{header}{content}"),
    );
}

/// A header value is one token: no whitespace, and nothing that could close the comment.
fn header_value(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '>' && *c != '=')
        .collect();
    if cleaned.is_empty() {
        "-".into()
    } else {
        cleaned
    }
}

/// Split a handoff file into its origin line (if it has one) and its body.
pub fn split_origin(content: &str) -> (Option<Origin>, &str) {
    let Some(rest) = content.strip_prefix(ORIGIN_PREFIX) else {
        return (None, content);
    };
    let (line, body) = rest.split_once('\n').unwrap_or((rest, ""));
    let Some(fields) = line.trim_end().strip_suffix(ORIGIN_SUFFIX) else {
        return (None, content);
    };

    let mut origin = Origin {
        session: String::new(),
        project: String::new(),
        written: String::new(),
    };
    for field in fields.split_whitespace() {
        match field.split_once('=') {
            Some(("session", v)) => origin.session = v.to_string(),
            Some(("project", v)) => origin.project = v.to_string(),
            Some(("written", v)) => origin.written = v.to_string(),
            _ => {}
        }
    }
    (Some(origin), body)
}

/// What `latest.md` holds for a session starting in `project_slug`.
#[derive(Debug, PartialEq, Eq)]
pub enum Latest {
    /// No file, an empty file, or a handoff written in a different project.
    None,
    /// A handoff this session should see. `origin` is absent for files written
    /// before the origin line existed.
    Handoff {
        origin: Option<Origin>,
        body: String,
    },
}

/// Read `latest.md` for a new session, consuming it when it is shown.
///
/// A handoff written in another project is left in place and not returned: it
/// is not this session's history, and its own project's next session may still
/// want it.
pub fn take_latest(paths: &ExoPaths, project_slug: &str) -> Latest {
    let latest = paths.handoffs_dir.join("latest.md");
    let Ok(content) = std::fs::read_to_string(&latest) else {
        return Latest::None;
    };
    let (origin, body) = split_origin(&content);
    if body.trim().is_empty() {
        let _ = std::fs::remove_file(&latest);
        return Latest::None;
    }
    if let Some(o) = &origin
        && o.project != "-"
        && !project_slug.is_empty()
        && o.project != project_slug
    {
        return Latest::None;
    }
    let body = body.to_string();
    let _ = std::fs::remove_file(&latest);
    Latest::Handoff { origin, body }
}

/// How old a session's own handoff may be and still describe what it is doing.
///
/// The handoff is rewritten at each compaction and session end. A session on a
/// large window can go months without either, and its subagents were then told
/// the session was doing whatever it had been doing at the last one: in one
/// case a sentence from eight months earlier.
const MAX_DIRECTION_AGE_SECS: u64 = 6 * 60 * 60;

fn is_recent(path: &std::path::Path, max_age_secs: u64) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age.as_secs() <= max_age_secs)
}

/// The "Working Direction" of this session's own handoff, for its subagents.
///
/// Reads `handoffs/<session_id>.md` and nothing else. With no session id, no
/// handoff yet for this session, or one too old to describe the present, the
/// subagent gets no handoff text: another session's is never a substitute, and
/// neither is this session's distant past.
pub fn session_direction(paths: &ExoPaths, session_id: &str, max_chars: usize) -> String {
    if session_id.is_empty() || session_id.contains(['/', '\\']) || session_id == "latest" {
        return String::new();
    }
    let path = paths.handoff_file(session_id);
    if !is_recent(&path, MAX_DIRECTION_AGE_SECS) {
        return String::new();
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    let (_, body) = split_origin(&content);

    let mut in_section = false;
    let mut direction_lines = Vec::new();
    for line in body.lines() {
        if line.starts_with("## Working Direction") {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if in_section {
            direction_lines.push(line);
        }
    }

    let direction = direction_lines.join("\n");
    let direction = direction.trim();
    if direction.is_empty() {
        truncate_at_boundary(body.trim(), max_chars)
    } else {
        truncate_at_boundary(direction, max_chars)
    }
}

/// Shorten `s` to at most `max` bytes without cutting a sentence or a word in
/// half, and say that it was shortened. A reader cannot judge half a value.
pub fn truncate_at_boundary(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let cut = crate::markdown::safe_truncate(s, max);
    let head = &s[..cut];

    // Last sentence end inside the budget: punctuation followed by whitespace.
    let sentence_end = head
        .char_indices()
        .filter(|(i, c)| {
            matches!(c, '.' | '!' | '?')
                && s[i + c.len_utf8()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
        })
        .map(|(i, c)| i + c.len_utf8())
        .next_back();

    let end = match sentence_end {
        Some(e) if e >= max / 2 => e,
        _ => head.rfind(char::is_whitespace).unwrap_or(cut),
    };
    format!("{} [truncated]", head[..end].trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(root: &std::path::Path) -> ExoPaths {
        let p = ExoPaths::at(root.to_path_buf());
        std::fs::create_dir_all(&p.handoffs_dir).unwrap();
        p
    }

    const B_HANDOFF: &str = "## Working Direction\n\nThe deploy needs one more thing from you: \
        the `gcp` loadout back in your defaults. My edit to your min config was blocked as a \
        permission change. In `~/.config/minimal/config.toml`, change \
        `default_loadouts = [\"broker\"]` to `default_loadouts = [\"broker\", \"gcp\"]`. \
        Tell me once that's saved.\n\n## Files Modified\n\n- infra/main.go\n";

    #[test]
    fn subagent_never_reads_another_sessions_handoff() {
        // The #28 incident: session B compacts and writes latest.md; session A,
        // which has no handoff of its own, starts a subagent.
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        std::fs::write(p.handoff_file("session-b"), B_HANDOFF).unwrap();
        write_latest(&p, "session-b", "gominimal--infra", B_HANDOFF);

        assert_eq!(session_direction(&p, "session-a", 400), "");
        assert_eq!(session_direction(&p, "", 400), "");
        // "latest" is not a session id, whatever the hook input says.
        assert_eq!(session_direction(&p, "latest", 400), "");
    }

    #[test]
    fn subagent_reads_its_own_sessions_direction() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        std::fs::write(
            p.handoff_file("session-a"),
            "## Working Direction\n\nIsolating lab-host users.\n\n## Files Modified\n\n- a.rs\n",
        )
        .unwrap();
        write_latest(&p, "session-b", "gominimal--infra", B_HANDOFF);

        assert_eq!(
            session_direction(&p, "session-a", 400),
            "Isolating lab-host users."
        );
    }

    #[test]
    fn session_id_cannot_escape_the_handoffs_dir() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        std::fs::write(dir.path().join("secret.md"), "## Working Direction\n\nx\n").unwrap();
        assert_eq!(session_direction(&p, "../secret", 400), "");
    }

    #[test]
    fn truncation_stops_at_a_sentence_and_says_so() {
        let direction = B_HANDOFF
            .split("\n\n")
            .nth(1)
            .expect("fixture has a direction paragraph");
        // A budget that lands inside the config value, as 400 bytes did in #28.
        let cut_inside = direction.find("\"gcp\"").unwrap() - 4;
        let out = truncate_at_boundary(direction, cut_inside);

        assert!(out.ends_with("permission change. [truncated]"), "{out}");
        assert!(!out.contains("default_loadouts"), "{out}");
        assert!(out.len() <= cut_inside + " [truncated]".len());
    }

    #[test]
    fn truncation_leaves_short_text_alone_and_survives_no_boundary() {
        assert_eq!(truncate_at_boundary("short", 400), "short");
        let unbroken = "x".repeat(50);
        let out = truncate_at_boundary(&unbroken, 20);
        assert_eq!(out, format!("{} [truncated]", "x".repeat(20)));
        // Multi-byte text is cut on a character boundary.
        let out = truncate_at_boundary(&"é".repeat(30), 21);
        assert!(out.starts_with("éééééééééé"));
    }

    #[test]
    fn origin_line_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        write_latest(
            &p,
            "abc-123",
            "workspace--karkinos",
            "## Working Direction\n\nbody\n",
        );
        let content = std::fs::read_to_string(p.handoffs_dir.join("latest.md")).unwrap();
        let (origin, body) = split_origin(&content);
        let origin = origin.expect("origin line");
        assert_eq!(origin.session, "abc-123");
        assert_eq!(origin.project, "workspace--karkinos");
        assert!(!origin.written.is_empty());
        assert_eq!(body, "## Working Direction\n\nbody\n");
    }

    #[test]
    fn origin_values_cannot_forge_fields_or_close_the_comment() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        write_latest(&p, "s1 project=other -->", "a b", "body\n");
        let content = std::fs::read_to_string(p.handoffs_dir.join("latest.md")).unwrap();
        let (origin, body) = split_origin(&content);
        let origin = origin.expect("origin line");
        assert_eq!(origin.project, "ab");
        assert_eq!(body, "body\n");
    }

    #[test]
    fn a_file_without_an_origin_line_is_all_body() {
        let (origin, body) = split_origin("## Working Direction\n\nold format\n");
        assert_eq!(origin, None);
        assert_eq!(body, "## Working Direction\n\nold format\n");
    }

    #[test]
    fn new_session_skips_a_handoff_from_another_project_and_leaves_it() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        write_latest(&p, "session-b", "gominimal--infra", B_HANDOFF);

        assert_eq!(take_latest(&p, "workspace--repro-lab"), Latest::None);
        assert!(p.handoffs_dir.join("latest.md").is_file());

        // The project it belongs to still gets it, once.
        match take_latest(&p, "gominimal--infra") {
            Latest::Handoff { origin, body } => {
                assert_eq!(origin.unwrap().session, "session-b");
                assert_eq!(body, B_HANDOFF);
            }
            Latest::None => panic!("own project should receive its handoff"),
        }
        assert!(!p.handoffs_dir.join("latest.md").exists());
        assert_eq!(take_latest(&p, "gominimal--infra"), Latest::None);
    }

    #[test]
    fn new_session_still_gets_a_file_written_before_the_origin_line_existed() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        std::fs::write(p.handoffs_dir.join("latest.md"), "old handoff\n").unwrap();
        assert_eq!(
            take_latest(&p, "workspace--anything"),
            Latest::Handoff {
                origin: None,
                body: "old handoff\n".into()
            }
        );
    }

    #[test]
    fn an_empty_latest_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        write_latest(&p, "s", "p", "  \n");
        assert_eq!(take_latest(&p, "p"), Latest::None);
        assert!(!p.handoffs_dir.join("latest.md").exists());
    }
    #[test]
    fn a_handoff_from_long_ago_is_not_the_sessions_current_direction() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(dir.path());
        let file = p.handoff_file("session-a");
        std::fs::write(
            &file,
            "## Working Direction\n\nBuilt and installed. Restart to see the status line.\n",
        )
        .unwrap();
        // Fresh: shown.
        assert!(session_direction(&p, "session-a", 400).starts_with("Built and installed"));

        // The same file, last written seven hours ago: not shown.
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 60 * 60);
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(session_direction(&p, "session-a", 400), "");
    }
}
