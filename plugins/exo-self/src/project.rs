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
        let dot_git = dir.join(".git");
        if dot_git.is_file() {
            // A linked worktree: its notes belong to the repository it was cut from.
            return Some(main_repo_of_worktree(&dir, &dot_git).unwrap_or(dir));
        }
        if dot_git.exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The main repository a linked worktree belongs to, read from the worktree's
/// `.git` file (`gitdir: <main>/.git/worktrees/<name>`).
///
/// Agent and workflow tooling runs sessions in throwaway worktrees. Keyed by
/// their own path, those sessions start with none of the project's notes and
/// leave their own in a directory nobody will open again
/// (`worktrees--agent-a317...`). Returns None for anything that is not a linked
/// worktree, such as a submodule, whose `.git` file points under `.git/modules/`.
fn main_repo_of_worktree(
    worktree: &std::path::Path,
    dot_git_file: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let content = std::fs::read_to_string(dot_git_file).ok()?;
    let gitdir = content
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    let gitdir = std::path::Path::new(gitdir);
    let gitdir = if gitdir.is_absolute() {
        gitdir.to_path_buf()
    } else {
        worktree.join(gitdir)
    };
    // <main>/.git/worktrees/<name>
    let worktrees_dir = gitdir.parent()?;
    let git_dir = worktrees_dir.parent()?;
    if worktrees_dir.file_name()? != "worktrees" || git_dir.file_name()? != ".git" {
        return None;
    }
    let main = git_dir.parent()?;
    // Normalise `..` segments from a relative gitdir when the path exists.
    Some(main.canonicalize().unwrap_or_else(|_| main.to_path_buf()))
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
        // Strip frontmatter — only inject a date · model header + prose to save tokens.
        // Model provenance matters now that the ecology is cross-lineage: a reader
        // should know whether a pattern claim is testimony from their own kind of
        // mind or from a different lineage (e.g. Opus vs Fable).
        let (fm, prose) = markdown::parse_frontmatter(&text);
        let prose = prose.trim();
        if prose.is_empty() {
            continue;
        }
        let date = fm.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let model = fm.get("model").and_then(|v| v.as_str()).unwrap_or("");

        let body = if idx < FULL_PROSE_NOTES {
            prose.to_string()
        } else {
            extract_marker_paragraphs(prose)
        };

        if body.is_empty() {
            // Older note with no Opinion/Surprise markers — drop without consuming budget.
            continue;
        }

        let note = match (date.is_empty(), model.is_empty()) {
            (false, false) => format!("**{date} · {model}**\n\n{body}"),
            (false, true) => format!("**{date}**\n\n{body}"),
            (true, false) => format!("**{model}**\n\n{body}"),
            (true, true) => body,
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

/// Count total session notes for a project, excluding underscore-prefixed special files
/// (`_digest.md`, `_summary.md`, `_legacy.md`). Used to prompt for a *first* digest on
/// projects that have accumulated many notes but were never curated — a case
/// `count_notes_after_digest` can't catch, since it returns 0 when no digest exists.
pub fn count_session_notes(paths: &ExoPaths, slug: &str) -> usize {
    let dir = paths.project_notes_dir(slug);
    if !dir.is_dir() {
        return 0;
    }
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
        .count()
}

/// Where a project's `_digest.md` stands relative to its session notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestState {
    /// Too few notes for a digest to be worth writing, or no notes at all.
    NotNeeded,
    /// Many notes and no digest.
    Missing { notes: usize },
    /// A digest exists and this many notes were written after it.
    Stale { new_notes: usize },
    /// A digest exists and covers nearly everything.
    Fresh,
}

/// Notes written after a digest before it is called stale. The loader stops
/// treating a digest as load-bearing at 2 newer notes; prompting that early
/// would nag every session.
pub const DIGEST_STALE_AFTER: usize = 5;
/// Notes a project accumulates before a first digest is suggested.
pub const DIGEST_FIRST_AFTER: usize = 25;

pub fn digest_state(paths: &ExoPaths, slug: &str) -> DigestState {
    if slug.is_empty() {
        return DigestState::NotNeeded;
    }
    let has_digest = paths.project_notes_dir(slug).join("_digest.md").is_file();
    if has_digest {
        let new_notes = count_notes_after_digest(paths, slug);
        if new_notes >= DIGEST_STALE_AFTER {
            DigestState::Stale { new_notes }
        } else {
            DigestState::Fresh
        }
    } else {
        let notes = count_session_notes(paths, slug);
        if notes >= DIGEST_FIRST_AFTER {
            DigestState::Missing { notes }
        } else {
            DigestState::NotNeeded
        }
    }
}

/// Every project whose digest is missing or stale, largest backlog first.
pub fn digest_backlog(paths: &ExoPaths) -> Vec<(String, DigestState)> {
    let Ok(entries) = std::fs::read_dir(&paths.per_project_dir) else {
        return Vec::new();
    };
    let mut backlog: Vec<(String, DigestState)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .map(|slug| {
            let state = digest_state(paths, &slug);
            (slug, state)
        })
        .filter(|(_, state)| {
            matches!(
                state,
                DigestState::Missing { .. } | DigestState::Stale { .. }
            )
        })
        .collect();
    let size = |state: &DigestState| match state {
        DigestState::Missing { notes } => *notes,
        DigestState::Stale { new_notes } => *new_notes,
        _ => 0,
    };
    backlog.sort_by(|a, b| size(&b.1).cmp(&size(&a.1)).then_with(|| a.0.cmp(&b.0)));
    backlog
}

/// The suggestion to write or refresh a digest, for a moment when the session
/// is pausing to reflect anyway (#24). None when the digest is fine.
pub fn digest_hint(state: DigestState) -> Option<String> {
    let lead = match state {
        DigestState::Missing { notes } => {
            format!("This project has {notes} session notes and no `_digest.md`.")
        }
        DigestState::Stale { new_notes } => {
            format!("This project's `_digest.md` is {new_notes} session notes behind.")
        }
        DigestState::NotNeeded | DigestState::Fresh => return None,
    };
    Some(format!(
        "{lead} The digest is what the next participant loads in place of older notes, \
         so this pause is a good time to write it: `/exo digest`. If now is the wrong \
         moment, tell the user it is due."
    ))
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
        if markdown::has_marker(trimmed, "Opinion") || markdown::has_marker(trimmed, "Surprise") {
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

/// The session notes in a project that belong to one session, oldest first.
///
/// Sessions do not keep to the one suggested file: they name notes after the
/// work, and write several in a sitting. A note belongs to the session when
///
///   * its frontmatter `session_id` is the session's id or its 8-character
///     short form, or
///   * its file name contains the short form, or
///   * it names no session at all and was written after the session started.
///
/// The last rule covers notes written without frontmatter. Two sessions open
/// in one project could each claim such a note; what is harvested from it is
/// the same either way and is de-duplicated, so only its session tag is in doubt.
pub fn session_note_paths(
    paths: &ExoPaths,
    slug: &str,
    session_id: &str,
    session_start: f64,
) -> Vec<std::path::PathBuf> {
    if slug.is_empty() || session_id.is_empty() {
        return Vec::new();
    }
    let short: String = session_id.chars().take(8).collect();
    let Ok(entries) = std::fs::read_dir(paths.project_notes_dir(slug)) else {
        return Vec::new();
    };
    let mut owned: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".md") || name.starts_with('_') || name == "scout.md" || !path.is_file()
        {
            continue;
        }
        let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (fm, _) = markdown::parse_frontmatter(&content);
        let stated = fm
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let is_owned = if !stated.is_empty() {
            stated == session_id || (stated.len() >= 8 && session_id.starts_with(stated))
        } else if short.len() == 8 && name.contains(short.as_str()) {
            true
        } else {
            session_start > 0.0
                && mtime
                    .duration_since(std::time::UNIX_EPOCH)
                    .is_ok_and(|d| d.as_secs_f64() > session_start)
                && !names_another_session(name)
        };
        if is_owned {
            owned.push((mtime, path));
        }
    }
    owned.sort();
    owned.into_iter().map(|(_, path)| path).collect()
}

/// True when a note's file name carries a session's short id in the suggested
/// position (`2026-11-23--ccff8de5.md`, `2026-09-21--c03fe411-k.md`).
fn names_another_session(file_name: &str) -> bool {
    file_name.split_once("--").is_some_and(|(_, rest)| {
        let id: String = rest.chars().take(8).collect();
        id.len() == 8 && id.chars().all(|c| c.is_ascii_hexdigit())
    })
}

/// The prose of every note the session wrote, apart from `except`, joined by
/// blank lines. Used with the suggested note's own prose to harvest markers.
pub fn other_session_prose(
    paths: &ExoPaths,
    slug: &str,
    session_id: &str,
    session_start: f64,
    except: &str,
) -> String {
    session_note_paths(paths, slug, session_id, session_start)
        .into_iter()
        .filter(|p| except.is_empty() || p != std::path::Path::new(except))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|content| {
            let (_, prose) = markdown::parse_frontmatter(&content);
            if prose.trim().is_empty() {
                content
            } else {
                prose
            }
        })
        .filter(|prose| !prose.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Modification time (Unix seconds) of the most recently written session note
/// for a project, or None when it has none. `_`-prefixed files are summaries,
/// not session notes, and are not counted.
pub fn newest_note_mtime(paths: &ExoPaths, slug: &str) -> Option<f64> {
    if slug.is_empty() {
        return None;
    }
    let entries = std::fs::read_dir(paths.project_notes_dir(slug)).ok()?;
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.ends_with(".md") && !n.starts_with('_'))
        })
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64()
        })
        .fold(None, |newest: Option<f64>, t| {
            Some(newest.map_or(t, |n| n.max(t)))
        })
}

/// The frontmatter a new session note should start with, filled in with what
/// the host knows (#23).
///
/// Sessions were told which fields to write but not their values, and often
/// name their own note files, so many notes carry no session id, project or
/// model, or a model spelled from memory. Handing over the exact lines makes
/// the identity fields a copy, not a recollection.
pub fn frontmatter_template(session_id: &str, project_slug: &str, model: Option<&str>) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d");
    let model_line = match model {
        Some(m) if !m.is_empty() => format!("model: \"{}\"", yaml_escape(m)),
        _ => "model: \"\"          # the serving model; a notice names it on your first prompt"
            .to_string(),
    };
    format!(
        "```yaml\n---\n\
        session_id: \"{}\"\n\
        date: \"{date}\"\n\
        project: \"{}\"\n\
        {model_line}\n\
        engagement:         # 1-5\n\
        engagement_mode:    # code, discussion, exploration, synthesis, design, debugging, ops, review\n\
        task_types: []      # e.g. [discussion, debugging, feature-dev, design, refactoring]\n\
        ---\n```",
        yaml_escape(session_id),
        yaml_escape(project_slug),
    )
}

/// The identity lines alone, for a checkpoint message.
pub fn identity_lines(session_id: &str, project_slug: &str, model: Option<&str>) -> String {
    let mut parts = Vec::new();
    if !session_id.is_empty() {
        parts.push(format!("`session_id: \"{}\"`", yaml_escape(session_id)));
    }
    if !project_slug.is_empty() {
        parts.push(format!("`project: \"{}\"`", yaml_escape(project_slug)));
    }
    if let Some(m) = model.filter(|m| !m.is_empty()) {
        parts.push(format!("`model: \"{}\"`", yaml_escape(m)));
    }
    parts.join(", ")
}

fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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

    /// Lay out `<tmp>/workspace/myrepo` with a linked worktree at `worktree_rel`,
    /// the way `git worktree add` does, and return (tmp, repo, worktree).
    fn repo_with_worktree(
        worktree_rel: &str,
        gitdir_line: impl Fn(&std::path::Path) -> String,
    ) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("workspace").join("myrepo");
        let admin = repo.join(".git").join("worktrees").join("wt");
        std::fs::create_dir_all(&admin).unwrap();
        let worktree = dir.path().join(worktree_rel);
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join(".git"), gitdir_line(&admin)).unwrap();
        (dir, repo, worktree)
    }

    #[test]
    fn slug_of_a_linked_worktree_is_its_main_repository() {
        // The shape that produced `worktrees--agent-a317f67b0b9d84195`.
        let (_dir, _repo, worktree) = repo_with_worktree("scratch/worktrees/agent-a317", |admin| {
            format!("gitdir: {}\n", admin.display())
        });
        assert_eq!(
            slug_from_input(&worktree.to_string_lossy()),
            "workspace--myrepo"
        );
        // And from a subdirectory inside the worktree.
        let sub = worktree.join("crates").join("core");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(slug_from_input(&sub.to_string_lossy()), "workspace--myrepo");
    }

    #[test]
    fn slug_of_a_worktree_with_a_relative_gitdir_resolves() {
        // git writes relative paths with `worktree.useRelativePaths`.
        let (_dir, _repo, worktree) = repo_with_worktree("workspace/myrepo-wt", |_| {
            "gitdir: ../myrepo/.git/worktrees/wt\n".to_string()
        });
        assert_eq!(
            slug_from_input(&worktree.to_string_lossy()),
            "workspace--myrepo"
        );
    }

    #[test]
    fn slug_of_a_submodule_or_unreadable_git_file_stays_its_own_path() {
        // Negative control: a `.git` file that is not a linked worktree keeps
        // the directory's own identity, as before.
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("parent").join("vendored");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(".git"), "gitdir: ../.git/modules/vendored\n").unwrap();
        assert_eq!(slug_from_input(&sub.to_string_lossy()), "parent--vendored");

        std::fs::write(sub.join(".git"), "not a gitdir line\n").unwrap();
        assert_eq!(slug_from_input(&sub.to_string_lossy()), "parent--vendored");
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

    #[test]
    fn extract_marker_paragraphs_keeps_every_marker_shape() {
        // #22: older notes written as `**Opinion: ...**` or `**Surprise:**`
        // contributed nothing to tiered loading.
        let prose = "Plain narrative.\n\n**Opinion: the corpus is the spec.**\n\n\
            More narrative.\n\n**Surprise:** the gate never ran.\n\n\
            **Surprises** in the plural are prose.";
        assert_eq!(
            extract_marker_paragraphs(prose),
            "**Opinion: the corpus is the spec.**\n\n**Surprise:** the gate never ran."
        );
    }

    #[test]
    fn newest_note_mtime_ignores_summaries_and_missing_projects() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ExoPaths::at(dir.path().to_path_buf());
        assert_eq!(newest_note_mtime(&paths, "absent"), None);
        assert_eq!(newest_note_mtime(&paths, ""), None);

        let notes = paths.project_notes_dir("p");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("_digest.md"), "summary").unwrap();
        assert_eq!(newest_note_mtime(&paths, "p"), None);

        // A custom-named note counts: sessions do not always use the suggested filename.
        std::fs::write(notes.join("2026-11-21-design-first.md"), "note").unwrap();
        let mtime = newest_note_mtime(&paths, "p").expect("a note exists");
        assert!((crate::state::now() - mtime).abs() < 60.0);
    }

    fn write_notes(paths: &ExoPaths, slug: &str, n: usize, prefix: &str) {
        let dir = paths.project_notes_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..n {
            std::fs::write(dir.join(format!("{prefix}-{i:03}.md")), "note").unwrap();
        }
    }

    #[test]
    fn digest_state_covers_each_case() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());

        assert_eq!(digest_state(&paths, ""), DigestState::NotNeeded);
        assert_eq!(digest_state(&paths, "absent"), DigestState::NotNeeded);

        write_notes(&paths, "small", DIGEST_FIRST_AFTER - 1, "a");
        assert_eq!(digest_state(&paths, "small"), DigestState::NotNeeded);

        write_notes(&paths, "big", DIGEST_FIRST_AFTER, "a");
        assert_eq!(
            digest_state(&paths, "big"),
            DigestState::Missing {
                notes: DIGEST_FIRST_AFTER
            }
        );

        // A digest written now covers everything before it.
        std::fs::write(paths.project_notes_dir("big").join("_digest.md"), "d").unwrap();
        assert_eq!(digest_state(&paths, "big"), DigestState::Fresh);

        // Notes after it make it stale once there are enough of them.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_notes(&paths, "big", DIGEST_STALE_AFTER - 1, "b");
        assert_eq!(digest_state(&paths, "big"), DigestState::Fresh);
        write_notes(&paths, "big", DIGEST_STALE_AFTER, "b");
        assert_eq!(
            digest_state(&paths, "big"),
            DigestState::Stale {
                new_notes: DIGEST_STALE_AFTER
            }
        );
    }

    #[test]
    fn digest_backlog_lists_only_projects_that_need_one_largest_first() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        write_notes(&paths, "fine", 3, "a");
        write_notes(&paths, "hundred", 100, "a");
        write_notes(&paths, "thirty", 30, "a");
        assert_eq!(
            digest_backlog(&paths),
            vec![
                ("hundred".to_string(), DigestState::Missing { notes: 100 }),
                ("thirty".to_string(), DigestState::Missing { notes: 30 }),
            ]
        );
    }

    #[test]
    fn digest_hint_is_silent_when_nothing_is_due() {
        assert_eq!(digest_hint(DigestState::NotNeeded), None);
        assert_eq!(digest_hint(DigestState::Fresh), None);
        let missing = digest_hint(DigestState::Missing { notes: 114 }).unwrap();
        assert!(missing.contains("114 session notes and no `_digest.md`"));
        assert!(missing.contains("/exo digest"));
        let stale = digest_hint(DigestState::Stale { new_notes: 7 }).unwrap();
        assert!(stale.contains("7 session notes behind"));
    }

    #[test]
    fn frontmatter_template_carries_the_values_and_parses_as_frontmatter() {
        let block = frontmatter_template(
            "c03fe411-2d1e-4f0a-9c11-000000000000",
            "gominimal--build-servers",
            Some("Fable 5.2"),
        );
        let note = block
            .trim_start_matches("```yaml\n")
            .trim_end_matches("```")
            .to_string()
            + "Prose.\n";
        let (fm, prose) = markdown::parse_frontmatter(&note);
        let get = |k: &str| fm.get(k).and_then(|v| v.as_str()).map(str::to_string);
        assert_eq!(
            get("session_id").as_deref(),
            Some("c03fe411-2d1e-4f0a-9c11-000000000000")
        );
        assert_eq!(get("project").as_deref(), Some("gominimal--build-servers"));
        assert_eq!(get("model").as_deref(), Some("Fable 5.2"));
        assert_eq!(get("date").map(|d| d.len()), Some(10));
        assert_eq!(prose.trim(), "Prose.");
    }

    #[test]
    fn frontmatter_template_without_a_known_model_leaves_it_empty_not_guessed() {
        let block = frontmatter_template("s", "p", None);
        assert!(block.contains("model: \"\""));
        let note = block
            .trim_start_matches("```yaml\n")
            .trim_end_matches("```")
            .to_string();
        let (fm, _) = markdown::parse_frontmatter(&note);
        assert_eq!(fm.get("model").and_then(|v| v.as_str()), Some(""));
    }

    #[test]
    fn identity_lines_omit_what_is_unknown() {
        assert_eq!(identity_lines("", "", None), "");
        assert_eq!(
            identity_lines("s1", "p", Some("Opus 5.5")),
            "`session_id: \"s1\"`, `project: \"p\"`, `model: \"Opus 5.5\"`"
        );
        assert_eq!(identity_lines("s1", "", Some("")), "`session_id: \"s1\"`");
    }

    #[test]
    fn session_notes_are_found_by_frontmatter_file_name_or_time() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ExoPaths::at(dir.path().to_path_buf());
        let notes = paths.project_notes_dir("p");
        std::fs::create_dir_all(&notes).unwrap();
        let sid = "c03fe411-2d1e-4f0a-9c11-000000000000";
        let started = crate::state::now() - 3600.0;
        let write = |name: &str, body: &str| std::fs::write(notes.join(name), body).unwrap();

        write(
            "2026-11-23--c03fe411.md",
            "---\nengagement: 4\n---\n\nsuggested file\n",
        );
        write(
            "2026-11-21-design-first.md",
            &format!("---\nsession_id: \"{sid}\"\n---\n\nfull id in frontmatter\n"),
        );
        write(
            "2026-11-22-landing.md",
            "---\nsession_id: \"c03fe411\"\n---\n\nshort id in frontmatter\n",
        );
        write(
            "2026-11-23-waiting-well.md",
            "no frontmatter, written this session\n",
        );
        // Not this session's:
        write(
            "2026-11-23--dee2313f.md",
            "---\nengagement: 5\n---\n\nanother session's suggested file\n",
        );
        write(
            "2026-11-20-theirs.md",
            "---\nsession_id: \"dee2313f-0000\"\n---\n\nnames another session\n",
        );
        write("_digest.md", "digest\n");

        let names: Vec<String> = session_note_paths(&paths, "p", sid, started)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec![
                "2026-11-21-design-first.md",
                "2026-11-22-landing.md",
                "2026-11-23--c03fe411.md",
                "2026-11-23-waiting-well.md",
            ]
        );

        let prose = other_session_prose(
            &paths,
            "p",
            sid,
            started,
            &notes.join("2026-11-23--c03fe411.md").to_string_lossy(),
        );
        assert!(prose.contains("full id in frontmatter"));
        assert!(prose.contains("short id in frontmatter"));
        assert!(prose.contains("no frontmatter, written this session"));
        assert!(!prose.contains("suggested file"));
        assert!(!prose.contains("another session"));
    }

    #[test]
    fn an_unlabelled_note_from_before_the_session_is_not_claimed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = ExoPaths::at(dir.path().to_path_buf());
        let notes = paths.project_notes_dir("p");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("2026-10-01-old.md"), "written long ago\n").unwrap();
        // The session starts after the note was written.
        let started = crate::state::now() + 60.0;
        assert!(session_note_paths(&paths, "p", "abcdef12-0000", started).is_empty());
        // And with no known start time nothing unlabelled is claimed.
        assert!(session_note_paths(&paths, "p", "abcdef12-0000", 0.0).is_empty());
        assert!(session_note_paths(&paths, "", "abcdef12-0000", 1.0).is_empty());
        assert!(session_note_paths(&paths, "p", "", 1.0).is_empty());
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
    fn injected_notes_carry_model_provenance() {
        // Cross-lineage ecology: the injected header must say whose testimony this is.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("provenance");
        std::fs::create_dir_all(&project_dir).unwrap();

        std::fs::write(
            project_dir.join("a.md"),
            "---\ndate: \"2026-05-29\"\nmodel: \"claude-fable-5\"\n---\n\nFable testimony.\n",
        )
        .unwrap();
        std::fs::write(
            project_dir.join("b.md"),
            "---\ndate: \"2026-05-28\"\n---\n\nNo model recorded.\n",
        )
        .unwrap();

        let out = load_recent_notes(&paths, "provenance", 6000);
        assert!(
            out.contains("**2026-05-29 · claude-fable-5**"),
            "model provenance missing from header: {out}"
        );
        // Notes without a model field keep the date-only header (no dangling separator).
        assert!(out.contains("**2026-05-28**"));
        assert!(!out.contains("2026-05-28 ·"));
    }

    #[test]
    fn count_session_notes_excludes_underscore_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        let project_dir = paths.project_notes_dir("counting");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(project_dir.join("a.md"), "x").unwrap();
        std::fs::write(project_dir.join("b.md"), "x").unwrap();
        std::fs::write(project_dir.join("_digest.md"), "x").unwrap();
        std::fs::write(project_dir.join("_summary.md"), "x").unwrap();
        assert_eq!(count_session_notes(&paths, "counting"), 2);
    }

    #[test]
    fn count_session_notes_zero_for_missing_project() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_for_root(dir.path());
        assert_eq!(count_session_notes(&paths, "nope"), 0);
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
