use crate::markdown;
use crate::paths::ExoPaths;

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
        // Skip frontmatter-only files (no prose content)
        let (_, prose) = markdown::parse_frontmatter(&text);
        if prose.trim().is_empty() {
            continue;
        }
        if total + text.len() > max_chars {
            let remaining = max_chars - total;
            if remaining > 100 {
                let mut truncated = text[..remaining].to_string();
                truncated.push_str("...");
                parts.push(truncated);
            }
            break;
        }
        total += text.len();
        parts.push(text);
    }

    parts.join("\n\n---\n\n")
}
