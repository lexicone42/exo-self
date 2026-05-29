use crate::markdown;
use crate::paths::ExoPaths;
use crate::state::SessionState;

/// Derive a stable project slug from the current working directory.
/// Prefers the git repo root so every subdirectory of a project shares one identity.
pub fn slug_from_cwd() -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    slug_for_dir(&cwd)
}

/// Last-two-path-components heuristic, joined by `--`.
/// e.g. /datar/workspace/my-project → workspace--my-project
/// Pure (no filesystem access) — used as the building block and the non-repo fallback.
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

/// Walk up from `start` looking for a `.git` entry (a directory for normal repos, a
/// file for worktrees/submodules). Returns the nearest ancestor (including `start`)
/// that contains one.
///
/// This is what lets every subdirectory of a project share a single ecology identity.
/// Without it the slug is cwd-dependent: a session launched from `repo/plugins/foo`
/// derived a separate slug (`plugins--foo`), fragmenting into an empty note-store and
/// silently losing all accumulated project continuity.
fn find_git_root(start: &str) -> Option<std::path::PathBuf> {
    if start.is_empty() {
        return None;
    }
    let mut dir = std::path::PathBuf::from(start);
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Resolve a project slug for a directory, preferring the git repo root so all
/// subdirectories of a project map to one identity. Falls back to the last-two-
/// path-components heuristic when the directory isn't inside a git repo.
fn slug_for_dir(dir: &str) -> String {
    match find_git_root(dir) {
        Some(root) => slug_from_path(&root.to_string_lossy()),
        None => slug_from_path(dir),
    }
}

/// Load project summary + recent session notes from per-project dir, capped at max_chars.
///
/// Tiered loading preserves accumulated high-signal traces across long project histories:
///   * Tier 1: `_summary.md` (curated project overview) — full prose if present.
///   * Tier 1b: `_digest.md` (human-or-Claude-curated rolling summary) — full prose
///     if present AND fresh (mtime newer than the 2nd-most-recent session note).
///   * Tier 2: latest `FULL_PROSE_NOTES` session notes — full prose.
///   * Tier 3: older notes — only paragraphs containing `**Opinion**` or `**Surprise**`
///     markers. Notes with no such markers contribute nothing and are skipped without
///     consuming budget. Skipped entirely when a fresh `_digest.md` is loaded, since
///     the digest is intended to cover everything older than the latest notes.
///
/// Without tiering, dense recent prose crowds out older Opinion/Surprise markers — the
/// explicit durable-identity and wrong-map traces — before they can be loaded. The full
/// prose remains on disk; the loader just doesn't surface it after the first few.
pub fn load_recent_notes(paths: &ExoPaths, slug: &str, max_chars: usize) -> String {
    /// Number of most-recent notes that get full-prose treatment. Older notes
    /// contribute only their Opinion/Surprise paragraphs (or are subsumed by a
    /// fresh `_digest.md` when present).
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

    // Gather session notes (newest first) — needed for both the digest freshness
    // check and the per-tier loading below.
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

    files.sort_by(|a, b| {
        let ma = std::fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let mb = std::fs::metadata(b)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        mb.cmp(&ma)
    });

    // Tier 1b: include _digest.md if it's fresher than the 2nd-most-recent note.
    // When the digest is fresh, it subsumes the older-tier marker scan — the digest
    // is the curated rolling summary and we don't want to double-surface its content.
    let mut have_fresh_digest = false;
    let digest_path = dir.join("_digest.md");
    if digest_path.is_file()
        && let Ok(digest_mtime) = std::fs::metadata(&digest_path).and_then(|m| m.modified())
    {
        // "Fresh" means newer than the second-most-recent note (or always-fresh when
        // there are 0 or 1 notes total — the digest covers all of history in that case).
        let second_newest_mtime = files
            .get(1)
            .and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        let fresh = match second_newest_mtime {
            Some(mt) => digest_mtime > mt,
            None => true,
        };
        if fresh && let Ok(text) = std::fs::read_to_string(&digest_path) {
            let (_, prose) = markdown::parse_frontmatter(&text);
            let prose = prose.trim();
            if !prose.is_empty() {
                let note = format!("**Project Digest**\n\n{prose}");
                total += note.len();
                parts.push(note);
                have_fresh_digest = true;
            }
        }
    }

    // Tiers 2 & 3: session notes by mtime descending, full prose then marker-only.
    // When a fresh digest is loaded, we still surface the latest few full-prose notes
    // (the digest covers history; the recent notes cover post-digest narrative) but
    // we skip the Tier 3 marker scan over older notes.
    for (idx, fp) in files.iter().enumerate() {
        if idx >= FULL_PROSE_NOTES && have_fresh_digest {
            // Older notes are subsumed by the digest.
            break;
        }

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

/// Count session notes written after `_digest.md` for a project.
///
/// Returns `0` when no digest exists or no session notes exist. Session-start uses this
/// to decide whether to surface a "consider re-digesting" hint — the loader's freshness
/// rule kicks in at 2 newer notes (point at which the digest stops being load-bearing),
/// but the hint should fire at a higher threshold to avoid nagging every session.
pub fn count_notes_after_digest(paths: &ExoPaths, slug: &str) -> usize {
    let dir = paths.project_notes_dir(slug);
    if !dir.is_dir() {
        return 0;
    }

    let Ok(digest_mtime) = std::fs::metadata(dir.join("_digest.md")).and_then(|m| m.modified())
    else {
        return 0;
    };

    let pattern = dir.join("*.md");
    let pattern_str = pattern.to_string_lossy();
    glob::glob(&pattern_str)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !n.starts_with('_'))
        })
        .filter(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .map(|m| m > digest_mtime)
                .unwrap_or(false)
        })
        .count()
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
        slug_for_dir(cwd)
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
    fn slug_from_path_is_pure_last_two() {
        assert_eq!(slug_from_path("/a/b/c/d"), "c--d");
        assert_eq!(
            slug_from_path("/datar/workspace/my-project"),
            "workspace--my-project"
        );
        assert_eq!(slug_from_path("/only"), "only");
        assert_eq!(slug_from_path(""), "");
    }

    #[test]
    fn slug_uses_git_root_from_subdir() {
        // <tmp>/parent/myrepo/.git, with a deep subdir below it.
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("parent").join("myrepo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let subdir = repo.join("plugins").join("inner");
        std::fs::create_dir_all(&subdir).unwrap();

        let root_slug = slug_from_path(&repo.to_string_lossy());
        // Root and deep subdir resolve to the SAME slug — no fragmentation.
        assert_eq!(slug_from_input(&subdir.to_string_lossy()), root_slug);
        assert_eq!(slug_from_input(&repo.to_string_lossy()), root_slug);
        // And it's the repo root's identity, not the subdir's components.
        assert_eq!(slug_from_input(&subdir.to_string_lossy()), "parent--myrepo");
        assert!(!slug_from_input(&subdir.to_string_lossy()).contains("inner"));
    }

    #[test]
    fn slug_detects_git_dir_at_cwd_itself() {
        // .git directly at the queried dir (repo root case).
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("grandparent").join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        assert_eq!(
            slug_from_input(&repo.to_string_lossy()),
            "grandparent--repo"
        );
    }

    #[test]
    fn slug_falls_back_to_path_without_git() {
        // No .git anywhere in the tree -> last-two-components heuristic.
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("alpha").join("beta");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(slug_from_input(&sub.to_string_lossy()), "alpha--beta");
    }

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

    /// Build a minimal ExoPaths anchored at `root`. Only `per_project_dir` matters
    /// for `load_recent_notes`, but the struct requires all fields populated.
    fn paths_for_root(root: &std::path::Path) -> ExoPaths {
        let root_buf = root.to_path_buf();
        ExoPaths {
            journal: root_buf.join("journal.md"),
            interests: root_buf.join("interests.md"),
            config: root_buf.join("config.json"),
            meta: root_buf.join("meta.json"),
            sessions_dir: root_buf.join("sessions"),
            handoffs_dir: root_buf.join("handoffs"),
            per_project_dir: root_buf.join("per-project"),
            shared_state: root_buf.join("shared-state.json"),
            context_window: root_buf.join("context-window.json"),
            synthesis: root_buf.join("synthesis.md"),
            sigils_dir: root_buf.join("sigils"),
            traces_dir: root_buf.join("traces"),
            root: root_buf,
        }
    }

    #[test]
    fn load_recent_notes_tiered_loading() {
        // Integration: verify that older notes contribute only their marker paragraphs.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("test-slug");
        std::fs::create_dir_all(&project_dir).unwrap();

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

    /// Helper for digest tests: write a note file. Caller pauses between writes to
    /// produce distinct ascending mtimes.
    fn write_note(project_dir: &std::path::Path, name: &str, date: &str, prose: &str) {
        let body = format!("---\ndate: \"{date}\"\n---\n\n{prose}\n");
        std::fs::write(project_dir.join(name), body).unwrap();
    }

    #[test]
    fn load_recent_notes_fresh_digest_subsumes_older_notes() {
        // Layout (oldest → newest by mtime):
        //   very_old.md  — pre-digest, OUTSIDE the latest-2 window
        //   older.md     — pre-digest, INSIDE the latest-2 window
        //   _digest.md   — fresh: mtime > files[1] (older.md)
        //   newer.md     — post-digest, INSIDE the latest-2 window
        //
        // Expected: digest + newer.md + older.md as full prose; very_old.md's
        // markers are suppressed because a fresh digest skips the Tier-3 scan.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("digest-fresh");
        std::fs::create_dir_all(&project_dir).unwrap();

        write_note(
            &project_dir,
            "very_old.md",
            "2026-02-01",
            "Ancient narrative.\n\n**Opinion** — Pre-digest position that should be hidden.",
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(
            &project_dir,
            "older.md",
            "2026-03-15",
            "Older narrative still in window.",
        );
        std::thread::sleep(std::time::Duration::from_millis(20));

        let digest_body =
            "---\ngenerated_at: 2026-05-10T00:00:00Z\n---\n\n## Through-Line\nDigest content here.";
        std::fs::write(project_dir.join("_digest.md"), digest_body).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        write_note(&project_dir, "newer.md", "2026-05-12", "Latest narrative.");

        let out = load_recent_notes(&paths, "digest-fresh", 6000);

        // Digest content is surfaced under its header.
        assert!(out.contains("**Project Digest**"), "missing digest header");
        assert!(out.contains("Digest content here."));

        // Latest two notes are full prose.
        assert!(out.contains("Latest narrative."));
        assert!(out.contains("Older narrative still in window."));

        // The pre-digest, out-of-window note's Opinion marker is NOT surfaced —
        // the fresh digest suppresses the Tier-3 marker scan.
        assert!(
            !out.contains("Pre-digest position that should be hidden."),
            "fresh digest should suppress older notes' marker scan"
        );
        assert!(
            !out.contains("Ancient narrative."),
            "very-old prose should be dropped entirely"
        );
    }

    #[test]
    fn load_recent_notes_stale_digest_ignored() {
        // When two or more newer notes exist after _digest.md, the digest is stale
        // and falls back to the standard tiered loading (older notes' markers
        // become visible again).
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("digest-stale");
        std::fs::create_dir_all(&project_dir).unwrap();

        // The digest comes first (oldest).
        let digest_body = "---\ngenerated_at: 2026-04-01T00:00:00Z\n---\n\nStale digest content.";
        std::fs::write(project_dir.join("_digest.md"), digest_body).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Then three notes after the digest. With ≥2 newer notes, the digest is stale.
        write_note(
            &project_dir,
            "older.md",
            "2026-04-15",
            "Older note.\n\n**Surprise** — A wrong-map worth keeping.",
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(&project_dir, "mid.md", "2026-04-25", "Mid note.");
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(&project_dir, "newest.md", "2026-05-12", "Newest note.");

        let out = load_recent_notes(&paths, "digest-stale", 6000);

        // The stale digest's content is NOT surfaced (the 2nd-newest note is newer
        // than the digest, so the freshness check fails).
        assert!(!out.contains("Stale digest content."));
        assert!(!out.contains("**Project Digest**"));

        // Standard tiered loading runs instead. Latest two notes are full prose;
        // the older note's Surprise marker is still surfaced from Tier 3.
        assert!(out.contains("Newest note."));
        assert!(out.contains("Mid note."));
        assert!(out.contains("**Surprise** — A wrong-map worth keeping."));
        assert!(
            !out.contains("Older note."),
            "older note's non-marker prose should not leak"
        );
    }

    #[test]
    fn count_notes_after_digest_returns_zero_without_digest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("no-digest");
        std::fs::create_dir_all(&project_dir).unwrap();
        write_note(&project_dir, "a.md", "2026-05-01", "First.");
        write_note(&project_dir, "b.md", "2026-05-02", "Second.");

        assert_eq!(count_notes_after_digest(&paths, "no-digest"), 0);
    }

    #[test]
    fn count_notes_after_digest_counts_only_newer_notes() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("with-digest");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Two notes pre-digest.
        write_note(&project_dir, "old1.md", "2026-04-01", "Old one.");
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(&project_dir, "old2.md", "2026-04-15", "Old two.");
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Digest written here.
        std::fs::write(project_dir.join("_digest.md"), "---\n---\n\nDigest body.").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Three notes after digest.
        write_note(&project_dir, "new1.md", "2026-05-01", "New one.");
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(&project_dir, "new2.md", "2026-05-05", "New two.");
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_note(&project_dir, "new3.md", "2026-05-10", "New three.");

        // Only post-digest notes are counted; the underscore-prefixed digest file
        // itself is excluded by the filter.
        assert_eq!(count_notes_after_digest(&paths, "with-digest"), 3);
    }

    #[test]
    fn count_notes_after_digest_returns_zero_for_missing_project() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        // Don't create any project dir.
        assert_eq!(count_notes_after_digest(&paths, "nonexistent"), 0);
    }

    #[test]
    fn load_recent_notes_digest_only_no_notes() {
        // Edge case: _digest.md exists but there are zero session notes.
        // The digest counts as always-fresh and is surfaced on its own.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("digest-only");
        std::fs::create_dir_all(&project_dir).unwrap();

        let digest_body = "---\ngenerated_at: 2026-05-10T00:00:00Z\n---\n\nSolo digest.";
        std::fs::write(project_dir.join("_digest.md"), digest_body).unwrap();

        let out = load_recent_notes(&paths, "digest-only", 6000);
        assert!(out.contains("**Project Digest**"));
        assert!(out.contains("Solo digest."));
    }
}
