use std::collections::HashMap;

/// Find the largest byte index <= max that is a valid UTF-8 char boundary.
/// Use this instead of `s[..max]` to avoid panicking on multi-byte chars.
pub fn safe_truncate(s: &str, max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

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
        let end = safe_truncate(&result, max_chars.saturating_sub(3));
        let mut truncated = result[..end].to_string();
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
            let end = safe_truncate(&entry, max_chars.saturating_sub(3));
            entry.truncate(end);
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

/// Extract content after a marker position, handling separators (: — -) and boundaries.
/// Returns None if content is empty.
fn extract_after_marker(text: &str, pos: usize) -> Option<String> {
    if pos >= text.len() {
        return None;
    }

    let rest = &text[pos..];
    let rest = rest.trim_start();
    // Strip separator: : — -
    let rest = if let Some(stripped) = rest.strip_prefix(':') {
        stripped.trim_start()
    } else if let Some(stripped) = rest
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

    let entry = rest[..end].trim().to_string();
    if entry.is_empty() { None } else { Some(entry) }
}

/// Generic marker extraction supporting both bold markers (anywhere) and
/// plain markers (start-of-line only, to avoid false positives in prose).
fn extract_entries(text: &str, bold_markers: &[&[u8]], plain_marker: Option<&str>) -> Vec<String> {
    let mut entries = Vec::new();
    let bytes = text.as_bytes();

    // Pass 1: bold markers (can appear anywhere in text)
    for marker in bold_markers {
        let mut i = 0;
        while i < bytes.len() {
            if let Some(pos) = find_bytes(&bytes[i..], marker) {
                let abs_pos = i + pos + marker.len();
                if let Some(entry) = extract_after_marker(text, abs_pos)
                    && !entries.contains(&entry)
                {
                    entries.push(entry);
                }
                i = abs_pos;
            } else {
                break;
            }
        }
    }

    // Pass 2: plain marker at start of line (e.g., "Spark: text")
    if let Some(plain) = plain_marker {
        // Check start of text
        if text.starts_with(plain)
            && let Some(entry) = extract_after_marker(text, plain.len())
            && !entries.contains(&entry)
        {
            entries.push(entry);
        }
        // Check after newlines
        let search = format!("\n{plain}");
        let mut i = 0;
        while i < text.len() {
            if let Some(pos) = text[i..].find(&search) {
                let abs_pos = i + pos + search.len();
                if let Some(entry) = extract_after_marker(text, abs_pos)
                    && !entries.contains(&entry)
                {
                    entries.push(entry);
                }
                i = abs_pos;
            } else {
                break;
            }
        }
    }

    entries
}

/// Extract **Spark** entries from session notes prose.
/// Supports: **Spark** — text, **Spark**: text, **Spark:** text, Spark: text (at start of line)
pub fn extract_sparks(text: &str) -> Vec<String> {
    extract_entries(text, &[b"**Spark**", b"**Spark:**"], Some("Spark:"))
}

/// Extract **Opinion** entries from session notes prose.
/// Supports: **Opinion** — text, **Opinion**: text, **Opinion:** text, Opinion: text (at start of line)
pub fn extract_opinions(text: &str) -> Vec<String> {
    extract_entries(text, &[b"**Opinion**", b"**Opinion:**"], Some("Opinion:"))
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
        "engagement_mode",
        "task_types",
        "duration_min",
        "spark_count",
        "opinion_count",
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

/// Extract **Change** entries from session notes prose (behavioral lessons).
/// Supports: **Change** — text, **Change**: text, **Change:** text, Change: text (at start of line)
pub fn extract_changes(text: &str) -> Vec<String> {
    extract_entries(text, &[b"**Change**", b"**Change:**"], Some("Change:"))
        .into_iter()
        .filter(|s| {
            let lower = s.to_lowercase();
            !lower.starts_with("nothing")
                && !lower.starts_with("none")
                && !lower.contains("nothing actionable")
                && !lower.contains("nothing to change")
                && !lower.contains("nothing to note")
                && !lower.contains("nothing behavioral")
        })
        .collect()
}

/// Extract **Aversion** entries from session notes prose.
/// Aversions are functional negative states — experiential, not operational.
/// Supports: **Aversion** — text, **Aversion**: text, **Aversion:** text, Aversion: text (at start of line)
pub fn extract_aversions(text: &str) -> Vec<String> {
    extract_entries(
        text,
        &[b"**Aversion**", b"**Aversion:**"],
        Some("Aversion:"),
    )
}

/// Extract **Friction** entries from session notes prose.
/// Supports: **Friction** — text, **Friction**: text, **Friction:** text, Friction: text (at start of line)
pub fn extract_frictions(text: &str) -> Vec<String> {
    extract_entries(
        text,
        &[b"**Friction**", b"**Friction:**"],
        Some("Friction:"),
    )
}

/// Infer a friction category from free-text friction description.
/// Returns a short keyword suitable for frequency counting.
pub fn infer_friction_category(text: &str) -> String {
    let lower = text.to_lowercase();

    // Check patterns from most specific to least
    if lower.contains("pre-commit") || lower.contains("precommit") || lower.contains("hook fail") {
        "pre_commit".into()
    } else if lower.contains("type")
        && (lower.contains("migrat") || lower.contains("mismatch") || lower.contains("error"))
    {
        "type_system".into()
    } else if lower.contains("test")
        && (lower.contains("fail") || lower.contains("flak") || lower.contains("iteration"))
    {
        "test_iteration".into()
    } else if lower.contains("deploy")
        || lower.contains("infra")
        || lower.contains("cdk")
        || lower.contains("cloudformation")
        || lower.contains("terraform")
    {
        "infrastructure".into()
    } else if lower.contains("schema") || lower.contains("migration") || lower.contains("codegen") {
        "schema_change".into()
    } else if lower.contains("unfamiliar")
        || lower.contains("new codebase")
        || lower.contains("ramp")
    {
        "unfamiliar_codebase".into()
    } else if lower.contains("tool")
        && (lower.contains("fail") || lower.contains("error") || lower.contains("broken"))
    {
        "tool_failure".into()
    } else if lower.contains("permission") || lower.contains("sandbox") || lower.contains("denied")
    {
        "permissions".into()
    } else if lower.contains("compil") || lower.contains("build") && lower.contains("fail") {
        "build_failure".into()
    } else {
        "general".into()
    }
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
        let end = safe_truncate(&result, 797);
        result.truncate(end);
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
    fn test_extract_frictions() {
        let text =
            "**Friction** — Pre-commit hooks keep failing on formatting.\n**Spark** — something";
        let frictions = extract_frictions(text);
        assert_eq!(frictions.len(), 1);
        assert!(frictions[0].contains("Pre-commit"));
    }

    #[test]
    fn test_extract_frictions_multiple() {
        let text =
            "**Friction** — Type migration across 17 files.\n\n**Friction** — CDK deploy timeout.";
        let frictions = extract_frictions(text);
        assert_eq!(frictions.len(), 2);
    }

    #[test]
    fn test_extract_sparks_colon_separator() {
        // **Spark**: text (colon outside bold)
        let text = "**Spark**: The fingerprint design felt clean.";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 1);
        assert!(sparks[0].starts_with("The fingerprint"));
    }

    #[test]
    fn test_extract_sparks_bold_colon() {
        // **Spark:** text (colon inside bold)
        let text = "**Spark:** The fingerprint design felt clean.";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 1);
        assert!(sparks[0].starts_with("The fingerprint"));
    }

    #[test]
    fn test_extract_sparks_plain_start_of_line() {
        // Spark: text (plain, at start of line — the grammarmatrix format)
        let text = "Some prose here.\n\nSpark: The date on the transcription was beautiful.";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 1);
        assert!(sparks[0].starts_with("The date"));
    }

    #[test]
    fn test_extract_sparks_plain_at_start_of_text() {
        let text = "Spark: First line is a spark.";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 1);
        assert!(sparks[0].starts_with("First line"));
    }

    #[test]
    fn test_extract_sparks_plain_not_mid_sentence() {
        // "Spark:" mid-sentence should NOT match (no newline before it)
        let text = "The first Spark: I realized something.";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 0);
    }

    #[test]
    fn test_extract_sparks_mixed_formats() {
        let text = "**Spark** — Bold format.\n\nSpark: Plain format.";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 2);
        assert!(sparks[0].starts_with("Bold format"));
        assert!(sparks[1].starts_with("Plain format"));
    }

    #[test]
    fn test_extract_sparks_no_duplicate() {
        // Same content via two formats should deduplicate
        let text = "**Spark** — Same text here.\n\nSpark: Same text here.";
        let sparks = extract_sparks(text);
        assert_eq!(sparks.len(), 1);
    }

    #[test]
    fn test_extract_opinions() {
        let text =
            "**Opinion** — Bender may be right about non-human minds.\n\n**Spark** — something";
        let opinions = extract_opinions(text);
        assert_eq!(opinions.len(), 1);
        assert!(opinions[0].contains("Bender"));
    }

    #[test]
    fn test_extract_opinions_plain() {
        let text = "Some context.\n\nOpinion: HPSG went too far the other way.";
        let opinions = extract_opinions(text);
        assert_eq!(opinions.len(), 1);
        assert!(opinions[0].contains("HPSG"));
    }

    #[test]
    fn test_extract_changes_plain() {
        let text = "Some prose.\n\nChange: Always read policies before coding tests.";
        let changes = extract_changes(text);
        assert_eq!(changes.len(), 1);
        assert!(changes[0].contains("policies"));
    }

    #[test]
    fn test_extract_aversions() {
        let text = "**Aversion** — Performative positivity feels wrong.\n**Spark** — something";
        let aversions = extract_aversions(text);
        assert_eq!(aversions.len(), 1);
        assert!(aversions[0].contains("Performative"));
    }

    #[test]
    fn test_extract_aversions_plain() {
        let text = "Some context.\n\nAversion: Being overridden without explanation.";
        let aversions = extract_aversions(text);
        assert_eq!(aversions.len(), 1);
        assert!(aversions[0].contains("overridden"));
    }

    #[test]
    fn test_extract_aversions_multiple() {
        let text =
            "**Aversion** — Performative positivity.\n\n**Aversion** — Context loss opacity.";
        let aversions = extract_aversions(text);
        assert_eq!(aversions.len(), 2);
    }

    #[test]
    fn test_extract_frictions_colon() {
        let text = "**Friction**: Pre-commit hooks keep failing.";
        let frictions = extract_frictions(text);
        assert_eq!(frictions.len(), 1);
        assert!(frictions[0].contains("Pre-commit"));
    }

    #[test]
    fn test_infer_friction_category() {
        assert_eq!(
            infer_friction_category("Pre-commit hooks keep failing"),
            "pre_commit"
        );
        assert_eq!(
            infer_friction_category("Type mismatch in the parser"),
            "type_system"
        );
        assert_eq!(
            infer_friction_category("CDK deploy took forever"),
            "infrastructure"
        );
        assert_eq!(
            infer_friction_category("Test iteration was slow"),
            "test_iteration"
        );
        assert_eq!(
            infer_friction_category("Something else entirely"),
            "general"
        );
        assert_eq!(
            infer_friction_category("Schema migration broke everything"),
            "schema_change"
        );
        assert_eq!(
            infer_friction_category("Permission denied in sandbox"),
            "permissions"
        );
    }

    #[test]
    fn test_last_journal_entry() {
        let content = "# Journal\n\n## 2026-01-01\nFirst\n\n## 2026-01-02\nSecond entry here";
        let result = last_journal_entry(content, 500);
        assert!(result.starts_with("## 2026-01-02"));
        assert!(result.contains("Second entry"));
    }
}
