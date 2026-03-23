//! Append-only trace files for the cognitive ecology.
//!
//! Each trace (spark, opinion, surprise, friction, lesson, aversion) is stored
//! as its own Markdown file with YAML frontmatter. No shared mutable state —
//! multiple Claudes can write simultaneously without data races.

use std::path::Path;

/// A parsed trace entry from a trace file.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    pub trace_type: String,
    pub text: String,
    pub project: String,
    pub timestamp: String,
    pub session_id: String,
    pub category: Option<String>,
}

/// Write a single trace to its own file in the traces directory.
/// Filename: `{type}--{timestamp}--{project_short}--{session_short}.md`
pub fn write_trace(
    traces_dir: &Path,
    trace_type: &str,
    text: &str,
    project: &str,
    session_id: &str,
    category: Option<&str>,
) {
    let now = chrono::Local::now();
    let ts_file = now.format("%Y-%m-%dT%H-%M-%S").to_string();
    let ts_front = now.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();

    // Short slugs for filename (avoid overly long names)
    let proj_short = if project.len() > 30 {
        &project[project.len() - 30..]
    } else {
        project
    };
    let sid_short = &session_id[..session_id.len().min(8)];

    let filename = format!("{trace_type}--{ts_file}--{proj_short}--{sid_short}.md");

    let mut frontmatter = format!(
        "---\ntype: \"{trace_type}\"\nproject: \"{project}\"\ntimestamp: \"{ts_front}\"\nsession_id: \"{session_id}\""
    );
    if let Some(cat) = category {
        frontmatter.push_str(&format!("\ncategory: \"{cat}\""));
    }
    frontmatter.push_str("\n---\n\n");

    let content = format!("{frontmatter}{text}\n");
    let path = traces_dir.join(&filename);
    let _ = std::fs::write(&path, &content);
}

/// Read all traces from the traces directory, optionally filtered.
/// Returns sorted by timestamp (oldest first).
pub fn read_traces(
    traces_dir: &Path,
    type_filter: Option<&str>,
    project_filter: Option<&str>,
) -> Vec<TraceEntry> {
    let pattern = traces_dir.join("*.md");
    let pattern_str = pattern.to_string_lossy();

    let mut entries: Vec<TraceEntry> = glob::glob(&pattern_str)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|path| parse_trace_file(&path))
        .filter(|e| type_filter.map(|t| e.trace_type == t).unwrap_or(true))
        .filter(|e| project_filter.map(|p| e.project == p).unwrap_or(true))
        .collect();

    // Sort by timestamp (oldest first — newest last for display)
    entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    entries
}

/// Parse a single trace file into a TraceEntry.
fn parse_trace_file(path: &Path) -> Option<TraceEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    let (fm, prose) = crate::markdown::parse_frontmatter(&content);

    let trace_type = fm.get("type").and_then(|v| v.as_str())?.to_string();
    let project = fm
        .get("project")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let timestamp = fm
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let session_id = fm
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let category = fm
        .get("category")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let text = prose.trim().to_string();
    if text.is_empty() {
        return None;
    }

    Some(TraceEntry {
        trace_type,
        text,
        project,
        timestamp,
        session_id,
        category,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_write_and_read_trace() {
        let dir = tempfile::tempdir().unwrap();
        let traces_dir = dir.path();

        write_trace(
            traces_dir,
            "spark",
            "The moment I realized the pattern.",
            "test-project",
            "abc12345",
            None,
        );

        let traces = read_traces(traces_dir, None, None);
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].trace_type, "spark");
        assert_eq!(traces[0].text, "The moment I realized the pattern.");
        assert_eq!(traces[0].project, "test-project");
    }

    #[test]
    fn test_filter_by_type() {
        let dir = tempfile::tempdir().unwrap();
        let traces_dir = dir.path();

        write_trace(traces_dir, "spark", "A spark", "proj", "s1", None);
        write_trace(traces_dir, "opinion", "An opinion", "proj", "s1", None);
        write_trace(traces_dir, "surprise", "A surprise", "proj", "s1", None);

        let sparks = read_traces(traces_dir, Some("spark"), None);
        assert_eq!(sparks.len(), 1);
        assert_eq!(sparks[0].trace_type, "spark");

        let all = read_traces(traces_dir, None, None);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_filter_by_project() {
        let dir = tempfile::tempdir().unwrap();
        let traces_dir = dir.path();

        write_trace(traces_dir, "spark", "Spark A", "alpha", "s1", None);
        write_trace(traces_dir, "spark", "Spark B", "beta", "s2", None);

        let alpha = read_traces(traces_dir, None, Some("alpha"));
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].text, "Spark A");
    }

    #[test]
    fn test_friction_with_category() {
        let dir = tempfile::tempdir().unwrap();
        let traces_dir = dir.path();

        write_trace(
            traces_dir,
            "friction",
            "Pre-commit hook failed",
            "proj",
            "s1",
            Some("pre_commit"),
        );

        let traces = read_traces(traces_dir, Some("friction"), None);
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].category.as_deref(), Some("pre_commit"));
    }

    #[test]
    fn test_traces_sorted_by_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let traces_dir = dir.path();

        // Write with explicit timestamps via manual files
        let files = [
            (
                "spark--2026-03-23T10-00-00--proj--s1.md",
                "---\ntype: \"spark\"\nproject: \"proj\"\ntimestamp: \"2026-03-23T10:00:00\"\nsession_id: \"s1\"\n---\n\nFirst",
            ),
            (
                "spark--2026-03-23T12-00-00--proj--s2.md",
                "---\ntype: \"spark\"\nproject: \"proj\"\ntimestamp: \"2026-03-23T12:00:00\"\nsession_id: \"s2\"\n---\n\nThird",
            ),
            (
                "spark--2026-03-23T11-00-00--proj--s3.md",
                "---\ntype: \"spark\"\nproject: \"proj\"\ntimestamp: \"2026-03-23T11:00:00\"\nsession_id: \"s3\"\n---\n\nSecond",
            ),
        ];
        for (name, content) in &files {
            fs::write(traces_dir.join(name), content).unwrap();
        }

        let traces = read_traces(traces_dir, None, None);
        assert_eq!(traces.len(), 3);
        assert_eq!(traces[0].text, "First");
        assert_eq!(traces[1].text, "Second");
        assert_eq!(traces[2].text, "Third");
    }
}
