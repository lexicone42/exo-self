use std::collections::HashMap;

/// Split journal content into entries by ## headings, return last N entries
pub fn last_journal_entries(content: &str, max_entries: usize, max_chars: usize) -> String {
    let entries: Vec<&str> = content.split("\n## ").collect();
    let count = entries.len();
    let start = count.saturating_sub(max_entries);

    let mut result = String::new();
    for (i, entry) in entries[start..].iter().enumerate() {
        if i > 0 || start > 0 {
            result.push_str("\n## ");
        }
        result.push_str(entry);
    }
    let result = result.trim().to_string();

    if result.len() > max_chars {
        let mut truncated = result[..max_chars - 3].to_string();
        truncated.push_str("...");
        truncated
    } else {
        result
    }
}

/// Get the last ## section from journal content (for subagent/teammate injection)
pub fn last_journal_entry(content: &str, max_chars: usize) -> String {
    let sections: Vec<&str> = content.split("\n## ").collect();
    if sections.len() > 1 {
        let mut entry = format!("## {}", sections.last().unwrap().trim());
        if entry.len() > max_chars {
            entry.truncate(max_chars - 3);
            entry.push_str("...");
        }
        entry
    } else {
        String::new()
    }
}

/// Filter interests to unchecked items only (lines starting with "- [ ]")
pub fn unchecked_interests(content: &str, max_items: usize) -> String {
    content
        .lines()
        .filter(|line| line.trim().starts_with("- [ ]"))
        .take(max_items)
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract **Spark** entries from session notes prose
pub fn extract_sparks(text: &str) -> Vec<String> {
    let mut sparks = Vec::new();
    let mut i = 0;
    let bytes = text.as_bytes();
    let marker = b"**Spark**";

    while i < bytes.len() {
        // Find next **Spark** marker
        if let Some(pos) = find_bytes(&bytes[i..], marker) {
            let abs_pos = i + pos + marker.len();
            // Skip separator: whitespace then — or -
            let rest = &text[abs_pos..];
            let rest = rest.trim_start();
            let rest = if let Some(stripped) = rest
                .strip_prefix('—')
                .or_else(|| rest.strip_prefix('\u{2014}'))
            {
                stripped.trim_start()
            } else if let Some(stripped) = rest.strip_prefix('-') {
                stripped.trim_start()
            } else {
                rest
            };

            // Collect until next **bold** marker, double newline, or end
            let mut end = rest.len();
            for (j, _) in rest.char_indices() {
                if j > 0 && rest[j..].starts_with("\n**") {
                    end = j;
                    break;
                }
                if j > 0 && rest[j..].starts_with("\n\n") {
                    end = j;
                    break;
                }
            }

            let spark = rest[..end].trim().to_string();
            if !spark.is_empty() {
                sparks.push(spark);
            }
            i = abs_pos + end;
        } else {
            break;
        }
    }
    sparks
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Parse YAML frontmatter from a markdown file.
/// Returns (frontmatter_map, prose_content)
pub fn parse_frontmatter(content: &str) -> (HashMap<String, serde_yaml::Value>, String) {
    if !content.starts_with("---") {
        return (HashMap::new(), content.to_string());
    }

    // Find closing ---
    if let Some(end) = content[3..].find("\n---") {
        let yaml_str = &content[4..3 + end]; // skip "---\n"
        let rest = &content[3 + end + 4..]; // skip "\n---\n"
        let map: HashMap<String, serde_yaml::Value> =
            serde_yaml::from_str(yaml_str).unwrap_or_default();
        (map, rest.trim_start_matches('\n').to_string())
    } else {
        (HashMap::new(), content.to_string())
    }
}

/// Render YAML frontmatter + prose back to a markdown string
pub fn render_frontmatter(map: &HashMap<String, serde_yaml::Value>, prose: &str) -> String {
    // Use ordered keys for consistent output
    let key_order = [
        "session_id",
        "date",
        "project",
        "model",
        "engagement",
        "task_types",
        "duration_min",
        "spark_count",
        "friction_density",
        "reflection_autonomy",
        "spark_density",
        "task_velocity",
    ];

    let mut yaml_lines = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Write known keys first in order
    for key in &key_order {
        if let Some(val) = map.get(*key) {
            yaml_lines.push(format_yaml_line(key, val));
            seen.insert(key.to_string());
        }
    }

    // Then any remaining keys
    let mut remaining: Vec<_> = map.keys().filter(|k| !seen.contains(k.as_str())).collect();
    remaining.sort();
    for key in remaining {
        if let Some(val) = map.get(key) {
            yaml_lines.push(format_yaml_line(key, val));
        }
    }

    let mut result = format!("---\n{}\n---\n", yaml_lines.join("\n"));
    if !prose.is_empty() {
        result.push('\n');
        result.push_str(prose);
        result.push('\n');
    }
    result
}

fn format_yaml_line(key: &str, val: &serde_yaml::Value) -> String {
    match val {
        serde_yaml::Value::Null => format!("{key}: null"),
        serde_yaml::Value::Bool(b) => format!("{key}: {b}"),
        serde_yaml::Value::Number(n) => format!("{key}: {n}"),
        serde_yaml::Value::String(s) => {
            if s.is_empty() {
                format!("{key}: \"\"")
            } else {
                format!("{key}: \"{s}\"")
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            if seq.is_empty() {
                format!("{key}: []")
            } else {
                let items: Vec<String> = seq
                    .iter()
                    .map(|v| match v {
                        serde_yaml::Value::String(s) => s.clone(),
                        other => format!("{other:?}"),
                    })
                    .collect();
                format!("{key}: [{}]", items.join(", "))
            }
        }
        other => {
            // Fallback: use serde_yaml serialization
            let s = serde_yaml::to_string(other).unwrap_or_default();
            format!("{key}: {}", s.trim())
        }
    }
}

/// Extract **Change** entries from session notes prose (behavioral lessons)
pub fn extract_changes(text: &str) -> Vec<String> {
    let mut changes = Vec::new();
    let mut i = 0;
    let bytes = text.as_bytes();
    let marker = b"**Change**";

    while i < bytes.len() {
        if let Some(pos) = find_bytes(&bytes[i..], marker) {
            let abs_pos = i + pos + marker.len();
            let rest = &text[abs_pos..];
            let rest = rest.trim_start();
            let rest = if let Some(stripped) = rest
                .strip_prefix('—')
                .or_else(|| rest.strip_prefix('\u{2014}'))
            {
                stripped.trim_start()
            } else if let Some(stripped) = rest.strip_prefix('-') {
                stripped.trim_start()
            } else {
                rest
            };

            // Collect until next **bold** marker, double newline, or end
            let mut end = rest.len();
            for (j, _) in rest.char_indices() {
                if j > 0 && rest[j..].starts_with("\n**") {
                    end = j;
                    break;
                }
                if j > 0 && rest[j..].starts_with("\n\n") {
                    end = j;
                    break;
                }
            }

            let change = rest[..end].trim().to_string();
            if !change.is_empty() {
                changes.push(change);
            }
            i = abs_pos + end;
        } else {
            break;
        }
    }
    changes
}

/// Extract synthesis key findings from synthesis.md
pub fn extract_synthesis_findings(content: &str) -> String {
    // Find "## Key Findings" section
    let marker = "## Key Findings\n";
    let Some(start) = content.find(marker) else {
        return String::new();
    };
    let after = &content[start + marker.len()..];

    // Find end of section (next ## or EOF)
    let end = after.find("\n## ").unwrap_or(after.len());
    let findings = after[..end].trim();

    if findings.is_empty() {
        return String::new();
    }

    // Also extract machine list header
    let mut result = String::new();
    if let Some(pos) = content.find("Machines: ") {
        let line_end = content[pos..].find('\n').unwrap_or(content.len() - pos);
        result.push_str(&content[pos..pos + line_end]);
        result.push_str("\n\n");
    }
    result.push_str(findings);

    // Cap at 800 chars
    if result.len() > 800 {
        result.truncate(797);
        result.push_str("...");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_journal_entries() {
        let content = "# Journal\n\n## Entry 1\nHello\n\n## Entry 2\nWorld\n\n## Entry 3\nFoo";
        let result = last_journal_entries(content, 2, 5000);
        assert!(result.contains("Entry 2"));
        assert!(result.contains("Entry 3"));
        assert!(!result.contains("Entry 1"));
    }

    #[test]
    fn test_unchecked_interests() {
        let content = "# Interests\n- [ ] Item 1\n- [x] Done\n- [ ] Item 2\n- [ ] Item 3";
        let result = unchecked_interests(content, 2);
        assert_eq!(result, "- [ ] Item 1\n- [ ] Item 2");
    }

    #[test]
    fn test_extract_sparks() {
        let text = "**Friction** — stuff\n**Spark** — The fingerprint design felt clean.\n**Change** — something";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 1);
        assert!(sparks[0].starts_with("The fingerprint"));
    }

    #[test]
    fn test_extract_changes() {
        let text = "**Spark** — something shiny\n**Change** — Should have profiled first before optimizing.\n**Friction** — stuff";
        let changes = extract_changes(text);
        assert_eq!(changes.len(), 1);
        assert!(changes[0].starts_with("Should have profiled"));
    }

    #[test]
    fn test_extract_changes_multiple() {
        let text = "**Change** — Read background agent outputs proactively.\n\n**Change** — Skip debug builds for DSP integration tests.";
        let changes = extract_changes(text);
        assert_eq!(changes.len(), 2);
        assert!(changes[0].contains("background agent"));
        assert!(changes[1].contains("debug builds"));
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\nsession_id: \"abc\"\ndate: \"2026-01-01\"\n---\n\nProse here";
        let (fm, prose) = parse_frontmatter(content);
        assert_eq!(fm.get("session_id").and_then(|v| v.as_str()), Some("abc"));
        assert_eq!(prose, "Prose here");
    }

    #[test]
    fn test_last_journal_entry() {
        let content = "# Journal\n\n## 2026-01-01\nFirst\n\n## 2026-01-02\nSecond entry here";
        let result = last_journal_entry(content, 500);
        assert!(result.starts_with("## 2026-01-02"));
        assert!(result.contains("Second entry"));
    }
}
