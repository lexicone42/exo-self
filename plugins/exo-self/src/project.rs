use crate::markdown;
use crate::paths::ExoPaths;
use crate::state::SessionState;

/// Derive a stable project slug from cwd.
/// Uses last 2 path components joined by -- for reasonable uniqueness.
/// e.g. /datar/workspace/my-project → workspace--my-project
pub fn slug_from_cwd() -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    slug_from_path(&cwd)
}

pub fn slug_from_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let trimmed = path.trim_end_matches('/');
    let parts: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 2 {
        format!("{}--{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else if let Some(last) = parts.last() {
        last.to_string()
    } else {
        String::new()
    }
}

/// Load project summary + recent session notes from per-project dir, capped at max_chars.
///
/// Tiered loading preserves accumulated high-signal traces across long project histories:
///   * Tier 1: `_summary.md` (curated project overview) — full prose if present.
///   * Tier 2: latest `FULL_PROSE_NOTES` session notes — full prose.
///   * Tier 3: older notes — only paragraphs containing `**Opinion**` or `**Surprise**`
///     markers. Notes with no such markers contribute nothing and are skipped without
///     consuming budget.
///
/// Without tiering, dense recent prose crowds out older Opinion/Surprise markers — the
/// explicit durable-identity and wrong-map traces — before they can be loaded. The full
/// prose remains on disk; the loader just doesn't surface it after the first few.
pub fn load_recent_notes(paths: &ExoPaths, slug: &str, max_chars: usize) -> String {
    /// Number of most-recent notes that get full-prose treatment. Older notes
    /// contribute only their Opinion/Surprise paragraphs.
    const FULL_PROSE_NOTES: usize = 2;

    let dir = paths.project_notes_dir(slug);
    if !dir.is_dir() {
        return String::new();
    }

    let mut parts = Vec::new();
    let mut total = 0;

    // Tier 1: Always include _summary.md if it exists (curated project overview)
    let summary_path = dir.join("_summary.md");
    if summary_path.is_file()
        && let Ok(text) = std::fs::read_to_string(&summary_path)
    {
        let (_, prose) = markdown::parse_frontmatter(&text);
        let prose = prose.trim();
        if !prose.is_empty() {
            let note = format!("**Project Summary**\n\n{prose}");
            total += note.len();
            parts.push(note);
        }
    }

    // Tiers 2 & 3: session notes by mtime descending, full prose then marker-only.
    let pattern = dir.join("*.md");
    let pattern_str = pattern.to_string_lossy();
    let mut files: Vec<_> = glob::glob(&pattern_str)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !n.starts_with('_'))
        })
        .collect();

    // Sort by mtime, newest first
    files.sort_by(|a, b| {
        let ma = std::fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mb = std::fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mb.cmp(&ma)
    });

    for (idx, fp) in files.iter().enumerate() {
        let text = std::fs::read_to_string(fp).unwrap_or_default();
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        // Strip frontmatter — only inject date header + prose to save tokens
        let (fm, prose) = markdown::parse_frontmatter(&text);
        let prose = prose.trim();
        if prose.is_empty() {
            continue;
        }
        let date = fm.get("date").and_then(|v| v.as_str()).unwrap_or("");

        let body = if idx < FULL_PROSE_NOTES {
            prose.to_string()
        } else {
            extract_marker_paragraphs(prose)
        };

        if body.is_empty() {
            // Older note with no Opinion/Surprise markers — drop without consuming budget.
            continue;
        }

        let note = if date.is_empty() {
            body
        } else {
            format!("**{date}**\n\n{body}")
        };

        if total + note.len() > max_chars {
            let remaining = max_chars - total;
            if remaining > 100 {
                let end = markdown::safe_truncate(&note, remaining);
                let mut truncated = note[..end].to_string();
                truncated.push_str("...");
                parts.push(truncated);
            }
            break;
        }
        total += note.len();
        parts.push(note);
    }

    parts.join("\n\n---\n\n")
}

/// Extract paragraphs containing `**Opinion**` or `**Surprise**` markers from prose.
///
/// Paragraphs are split on blank lines (markdown paragraph boundary). Paragraphs without
/// either marker are dropped; matching paragraphs are kept verbatim and rejoined.
/// Returns an empty string when no marker paragraphs are present.
fn extract_marker_paragraphs(prose: &str) -> String {
    let mut kept = Vec::new();
    for paragraph in prose.split("\n\n") {
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains("**Opinion**") || trimmed.contains("**Surprise**") {
            kept.push(trimmed.to_string());
        }
    }
    kept.join("\n\n")
}

/// Remove empty session notes files (frontmatter-only, no prose) from all per-project dirs.
/// Called at both session-start and session-end to minimize accumulation of empties
/// from short sessions where hooks don't fire cleanly.
pub fn cleanup_empty_notes(paths: &ExoPaths) {
    let Ok(projects) = std::fs::read_dir(&paths.per_project_dir) else {
        return;
    };

    let skip = ["_legacy.md", "_summary.md", "sessions.md"];

    for project_entry in projects.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        let Ok(notes) = std::fs::read_dir(&project_path) else {
            continue;
        };

        for note_entry in notes.flatten() {
            let note_path = note_entry.path();
            let Some(name) = note_path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            if !name.ends_with(".md") || skip.contains(&name) {
                continue;
            }

            // Only check small files — real notes with prose are larger
            let Ok(meta) = std::fs::metadata(&note_path) else {
                continue;
            };
            if meta.len() > 500 {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&note_path) else {
                continue;
            };

            let (_, prose) = markdown::parse_frontmatter(&content);
            if prose.trim().is_empty() {
                let _ = std::fs::remove_file(&note_path);
            }
        }
    }

    // Remove empty project directories
    if let Ok(projects) = std::fs::read_dir(&paths.per_project_dir) {
        for entry in projects.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // rmdir only succeeds on empty dirs
                let _ = std::fs::remove_dir(&path);
            }
        }
    }
}

/// Derive project slug from hook input, preferring cwd from Claude Code over process CWD.
pub fn slug_from_input(cwd: &str) -> String {
    if !cwd.is_empty() {
        slug_from_path(cwd)
    } else {
        slug_from_cwd()
    }
}

/// Detect whether the user (Claude) has written session notes during this session.
/// Checks per-session notes file for prose content, then falls back to journal mtime.
pub fn detect_wrote_notes(state: &SessionState, paths: &ExoPaths, session_start: f64) -> bool {
    // Per-session notes file
    if !state.session_notes_path.is_empty()
        && let Ok(content) = std::fs::read_to_string(&state.session_notes_path)
    {
        let (_, prose) = markdown::parse_frontmatter(&content);
        if !prose.trim().is_empty() {
            return true;
        }
    }

    // Journal mtime fallback
    if session_start > 0.0 {
        return file_modified_after(&paths.journal, session_start);
    }

    false
}

pub fn file_modified_after(path: &std::path::Path, after: f64) -> bool {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64()
                > after
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_markers_empty() {
        assert_eq!(extract_marker_paragraphs(""), "");
    }

    #[test]
    fn extract_markers_no_markers() {
        let prose = "Just some regular prose.\n\nAnother paragraph here.";
        assert_eq!(extract_marker_paragraphs(prose), "");
    }

    #[test]
    fn extract_markers_single_opinion() {
        let prose =
            "Intro paragraph.\n\n**Opinion** — Tools reflect the builder.\n\nClosing thought.";
        let result = extract_marker_paragraphs(prose);
        assert_eq!(result, "**Opinion** — Tools reflect the builder.");
    }

    #[test]
    fn extract_markers_single_surprise() {
        let prose = "Some context.\n\n**Surprise** — The map was wrong here.";
        let result = extract_marker_paragraphs(prose);
        assert_eq!(result, "**Surprise** — The map was wrong here.");
    }

    #[test]
    fn extract_markers_preserves_order() {
        let prose = "Lead-in.\n\n**Surprise** — First surprise.\n\nIntermediate prose.\n\n\
             **Opinion** — A take.\n\n**Surprise** — Second surprise.\n\nOutro.";
        let result = extract_marker_paragraphs(prose);
        let expected = "**Surprise** — First surprise.\n\n\
                        **Opinion** — A take.\n\n\
                        **Surprise** — Second surprise.";
        assert_eq!(result, expected);
    }

    #[test]
    fn extract_markers_ignores_other_marker_types() {
        // Spark, Aversion, Friction, Change are tracked elsewhere — not durable identity traces.
        let prose = "**Spark** — Fun moment.\n\n**Aversion** — Disliked the pattern.\n\n\
                     **Friction** — Tooling fought back.\n\n**Change** — Should profile first.";
        assert_eq!(extract_marker_paragraphs(prose), "");
    }

    #[test]
    fn extract_markers_multiline_paragraph() {
        // Marker paragraphs sometimes span multiple lines without blank-line breaks.
        let prose = "**Opinion** — A long take\nthat spans\nseveral lines.\n\nNext paragraph.";
        let result = extract_marker_paragraphs(prose);
        assert_eq!(
            result,
            "**Opinion** — A long take\nthat spans\nseveral lines."
        );
    }

    #[test]
    fn extract_markers_mixed_with_unrelated_paragraphs() {
        let prose = "Opening.\n\nSomething about the code.\n\n\
                     **Opinion** — Worth keeping.\n\nMore narrative.\n\n\
                     **Spark** — Fun but transient.\n\n**Surprise** — Also worth keeping.";
        let result = extract_marker_paragraphs(prose);
        let expected = "**Opinion** — Worth keeping.\n\n**Surprise** — Also worth keeping.";
        assert_eq!(result, expected);
    }

    #[test]
    fn load_recent_notes_tiered_loading() {
        // Integration: verify that older notes contribute only their marker paragraphs.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let per_project = root.join("per-project");
        let project_dir = per_project.join("test-slug");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Build a minimal ExoPaths pointing at the temp tree. Only per_project_dir
        // is consulted by load_recent_notes, but all fields must be populated.
        let paths = ExoPaths {
            journal: root.join("journal.md"),
            interests: root.join("interests.md"),
            config: root.join("config.json"),
            meta: root.join("meta.json"),
            sessions_dir: root.join("sessions"),
            handoffs_dir: root.join("handoffs"),
            per_project_dir: per_project,
            shared_state: root.join("shared-state.json"),
            context_window: root.join("context-window.json"),
            synthesis: root.join("synthesis.md"),
            sigils_dir: root.join("sigils"),
            traces_dir: root.join("traces"),
            root: root.clone(),
        };

        // Helper: write a note. Caller writes in oldest→newest order with brief sleeps
        // between writes so mtimes are naturally ordered without needing an external
        // filetime crate.
        let write_note = |name: &str, date: &str, prose: &str| {
            let body = format!("---\ndate: \"{date}\"\n---\n\n{prose}\n");
            std::fs::write(project_dir.join(name), body).unwrap();
        };

        // Write oldest first so mtimes ascend; the loader sorts descending.
        write_note(
            "ancient.md",
            "2026-03-01",
            "Ancient chatter.\n\nNo markers anywhere.",
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(
            "older.md",
            "2026-04-20",
            "Old chatter that should drop.\n\n**Surprise** — Important wrong-map.\n\n\
             More chatter.\n\n**Opinion** — Worth surfacing.",
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(
            "second.md",
            "2026-05-10",
            "Second-newest narrative.\n\nNo markers in this one.",
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(
            "newest.md",
            "2026-05-12",
            "Latest narrative here.\n\n**Opinion** — Fresh take.",
        );

        let out = load_recent_notes(&paths, "test-slug", 6000);

        // Latest two notes: full prose (chatter present).
        assert!(
            out.contains("Latest narrative here."),
            "newest prose missing"
        );
        assert!(
            out.contains("Second-newest narrative."),
            "second prose missing"
        );
        assert!(
            out.contains("No markers in this one."),
            "second-note prose missing"
        );

        // Older note: only marker paragraphs.
        assert!(out.contains("**Surprise** — Important wrong-map."));
        assert!(out.contains("**Opinion** — Worth surfacing."));
        assert!(
            !out.contains("Old chatter that should drop."),
            "older note's non-marker prose leaked through"
        );
        assert!(
            !out.contains("More chatter."),
            "older note's intermediate prose leaked through"
        );

        // Ancient note with no markers: dropped entirely.
        assert!(!out.contains("Ancient chatter."));
    }
}
