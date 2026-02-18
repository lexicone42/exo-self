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

/// Load the N most recent session note files from per-project dir, capped at max_chars
pub fn load_recent_notes(paths: &ExoPaths, slug: &str, max_chars: usize) -> String {
    let dir = paths.project_notes_dir(slug);
    if !dir.is_dir() {
        return String::new();
    }

    let pattern = dir.join("*.md");
    let pattern_str = pattern.to_string_lossy();
    let mut files: Vec<_> = glob::glob(&pattern_str)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
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

    let mut parts = Vec::new();
    let mut total = 0;
    for fp in &files {
        if parts.len() >= 5 {
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
        let note = if !date.is_empty() {
            format!("**{date}**\n\n{prose}")
        } else {
            prose.to_string()
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

/// Remove empty session notes files (frontmatter-only, no prose) from all per-project dirs.
/// Called at both session-start and session-end to minimize accumulation of empties
/// from short sessions where hooks don't fire cleanly.
pub fn cleanup_empty_notes(paths: &ExoPaths) {
    let Ok(projects) = std::fs::read_dir(&paths.per_project_dir) else {
        return;
    };

    let skip = ["_legacy.md", "sessions.md"];

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
        && let Ok(content) = std::fs::read_to_string(&state.session_notes_path) {
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
