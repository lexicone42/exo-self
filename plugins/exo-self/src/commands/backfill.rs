use crate::markdown;
use crate::meta::{Lesson, Meta};
use crate::paths::ExoPaths;
use std::path::Path;

/// Scan all session notes and backfill lessons (Change entries) into meta.json.
/// Idempotent: uses the same dedup logic as session_end.
pub fn run() {
    let paths = ExoPaths::new();
    let mut meta = Meta::load(&paths.meta);

    let before = meta.lessons.len();
    let mut scanned = 0u32;

    // Walk all per-project directories
    let Ok(projects) = std::fs::read_dir(&paths.per_project_dir) else {
        eprintln!("backfill: no per-project directory found");
        return;
    };

    for project_entry in projects.flatten() {
        let project_dir = project_entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let project_slug = project_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let Ok(files) = std::fs::read_dir(&project_dir) else {
            continue;
        };

        for file_entry in files.flatten() {
            let path = file_entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Only session note files (YYYY-MM-DD--*.md)
            if !name.ends_with(".md") || !name.starts_with("2026-") {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(&path) {
                let (_fm, prose) = markdown::parse_frontmatter(&content);
                if prose.is_empty() {
                    continue;
                }
                scanned += 1;

                let session_id = extract_session_id(&name);
                extract_and_store_lessons(&prose, &project_slug, &session_id, &path, &mut meta);
            }
        }
    }

    // Sort by timestamp (most recent last) then cap at 20
    meta.lessons.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    let len = meta.lessons.len();
    if len > 20 {
        meta.lessons = meta.lessons.split_off(len - 20);
    }

    meta.save(&paths.meta);

    let added = meta.lessons.len() - before;
    println!(
        "backfill: scanned {} files, added {} lessons (total: {})",
        scanned,
        added,
        meta.lessons.len()
    );

    // Show them
    for lesson in &meta.lessons {
        let short = if lesson.text.len() > 100 {
            format!("{}...", &lesson.text[..97])
        } else {
            lesson.text.clone()
        };
        println!("  [{}] {}", lesson.project, short);
    }
}

fn extract_session_id(filename: &str) -> String {
    // "2026-03-02--c254693a.md" -> "c254693a"
    filename
        .strip_suffix(".md")
        .and_then(|s| s.split("--").nth(1))
        .unwrap_or("unknown")
        .to_string()
}

fn extract_and_store_lessons(
    prose: &str,
    project_slug: &str,
    session_id: &str,
    path: &Path,
    meta: &mut Meta,
) {
    let changes = markdown::extract_changes(prose);
    if changes.is_empty() {
        return;
    }

    // Use file mtime as timestamp approximation
    let timestamp = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
        })
        .unwrap_or_else(|| "unknown".into());

    for change_text in &changes {
        let dedup_end = markdown::safe_truncate(change_text, 100);
        let dedup_key = change_text[..dedup_end].to_lowercase();
        let is_dup = meta.lessons.iter().any(|l| {
            let l_end = markdown::safe_truncate(&l.text, 100);
            l.text[..l_end].to_lowercase() == dedup_key
        });
        if !is_dup {
            meta.lessons.push(Lesson {
                text: change_text.clone(),
                project: project_slug.to_string(),
                timestamp: timestamp.clone(),
                session_id: session_id.to_string(),
            });
        }
    }
}
