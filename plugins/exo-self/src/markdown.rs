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

/// Byte offsets just past each bold occurrence of marker `name` in `text`,
/// with what follows the name.
///
/// A bold marker is `**Name` where the next character does not continue the
/// word, so `**Surprise` matches and `**Surprises` does not. Three shapes are
/// in use in real notes and all are accepted (#22):
///
///   * `**Opinion** text` and `**Opinion:** text`: the bold span is the marker alone.
///   * `**Opinion: a whole sentence.**`: the bold span covers the entry.
///   * `**Surprise (the good one)** text` and `**Surprise #2:** text`: a
///     qualifier sits between the name and the end of the bold span.
///
/// The last two are recognised only when a colon, dash, comma, parenthesis or
/// `#` follows the name, so bold phrases that merely start with a marker word
/// are left alone.
fn bold_marker_sites(text: &str, name: &str) -> Vec<(usize, MarkerShape)> {
    let needle = format!("**{name}");
    let mut sites = Vec::new();
    let mut from = 0;
    while let Some(pos) = text[from..].find(&needle) {
        let after_name = from + pos + needle.len();
        from = after_name;
        let rest = &text[after_name..];
        let Some(next) = rest.chars().next() else {
            break;
        };
        if next.is_alphanumeric() || next == '_' {
            continue;
        }
        if rest.starts_with("**") {
            sites.push((after_name + 2, MarkerShape::Alone));
        } else if rest.starts_with(":**") {
            sites.push((after_name + 3, MarkerShape::Alone));
        } else if [":", " \u{2014}", " -", " (", ",", " #"]
            .iter()
            .any(|q| rest.starts_with(q))
        {
            sites.push((after_name, MarkerShape::Spanning));
        }
        // Anything else (`**Change log**`, `**Phase 1**`) is ordinary bold text.
    }
    sites
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkerShape {
    /// The bold span closed right after the name.
    Alone,
    /// The bold span is still open: its closing `**` is inside the entry text.
    Spanning,
}

/// True when `text` carries a bold marker `name` in any accepted shape.
pub fn has_marker(text: &str, name: &str) -> bool {
    !bold_marker_sites(text, name).is_empty()
}

/// Marker extraction: bold markers anywhere in the text, plus `Name:` at the
/// start of a line (start-of-line only, to avoid false positives in prose).
fn extract_entries(text: &str, name: &str) -> Vec<String> {
    let mut entries = Vec::new();

    // Pass 1: bold markers (can appear anywhere in text)
    for (pos, shape) in bold_marker_sites(text, name) {
        let Some(mut entry) = extract_after_marker(text, pos) else {
            continue;
        };
        if shape == MarkerShape::Spanning {
            // Drop the `**` that closes the span the marker opened.
            if let Some(close) = entry.find("**") {
                entry.replace_range(close..close + 2, "");
            }
            entry = entry.trim_start_matches(',').trim().to_string();
            if entry.is_empty() {
                continue;
            }
        }
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }

    // Pass 2: plain marker at start of line (e.g., "Spark: text")
    let plain = format!("{name}:");
    if text.starts_with(&plain)
        && let Some(entry) = extract_after_marker(text, plain.len())
        && !entries.contains(&entry)
    {
        entries.push(entry);
    }
    let search = format!("\n{plain}");
    let mut i = 0;
    while let Some(pos) = text[i..].find(&search) {
        let abs_pos = i + pos + search.len();
        if let Some(entry) = extract_after_marker(text, abs_pos)
            && !entries.contains(&entry)
        {
            entries.push(entry);
        }
        i = abs_pos;
    }

    entries
}

/// Extract **Spark** entries from session notes prose.
/// Supports: **Spark** — text, **Spark**: text, **Spark:** text, Spark: text (at start of line)
pub fn extract_sparks(text: &str) -> Vec<String> {
    extract_entries(text, "Spark")
}

/// Extract **Opinion** entries from session notes prose.
/// Supports: **Opinion** — text, **Opinion**: text, **Opinion:** text, Opinion: text (at start of line)
pub fn extract_opinions(text: &str) -> Vec<String> {
    extract_entries(text, "Opinion")
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
        "scouted",
        "spark_density",
        "task_velocity",
        "phase_count",
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
    extract_entries(text, "Change")
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
    extract_entries(text, "Aversion")
}

/// Extract **Surprise** entries from session notes prose.
/// Negative stigmergy: traces that warn future participants the map doesn't match the territory.
pub fn extract_surprises(text: &str) -> Vec<String> {
    extract_entries(text, "Surprise")
}

/// Extract **Friction** entries from session notes prose.
/// Supports: **Friction** — text, **Friction**: text, **Friction:** text, Friction: text (at start of line)
pub fn extract_frictions(text: &str) -> Vec<String> {
    extract_entries(text, "Friction")
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
    } else if lower.contains("lock") || lower.contains("contention") || lower.contains("busy") {
        "lock_contention".into()
    } else if lower.contains("network") || lower.contains("timeout") || lower.contains("connect") {
        "network".into()
    } else if lower.contains("syntax") || lower.contains("parse error") {
        "syntax_error".into()
    } else {
        "unclassified".into()
    }
}

/// A parsed phase entry with structured engagement data.
#[derive(Debug, Clone)]
pub struct Phase {
    pub engagement: Option<f64>,
    pub task_types: Vec<String>,
    pub description: String,
}

/// Extract **Phase** entries from session notes prose.
/// Supports: **Phase** (engagement: 4, [debugging]) — text, **Phase:** text, Phase: text
pub fn extract_phases(text: &str) -> Vec<String> {
    extract_entries(text, "Phase")
}

/// Parse structured data from a phase entry string.
/// Expected format: "(engagement: 4, [debugging, design]) — Description text"
/// or just plain description text without structured prefix.
pub fn parse_phase(entry: &str) -> Phase {
    if !entry.starts_with('(') {
        return Phase {
            engagement: None,
            task_types: Vec::new(),
            description: entry.to_string(),
        };
    }

    let Some(paren_end) = entry.find(')') else {
        return Phase {
            engagement: None,
            task_types: Vec::new(),
            description: entry.to_string(),
        };
    };

    let inner = &entry[1..paren_end];
    let after = entry[paren_end + 1..].trim();
    // Strip separator after closing paren
    let description = after
        .strip_prefix('—')
        .or_else(|| after.strip_prefix('\u{2014}'))
        .or_else(|| after.strip_prefix('-'))
        .or_else(|| after.strip_prefix(':'))
        .unwrap_or(after)
        .trim()
        .to_string();

    let mut engagement = None;
    let mut task_types = Vec::new();

    // Parse engagement: N
    if let Some(eng_pos) = inner.find("engagement:") {
        let after_eng = inner[eng_pos + 11..].trim_start();
        let num_end = after_eng
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(after_eng.len());
        if let Ok(n) = after_eng[..num_end].parse::<f64>() {
            engagement = Some(n);
        }
    }

    // Parse [task, types]
    if let Some(bracket_start) = inner.find('[')
        && let Some(bracket_end) = inner[bracket_start..].find(']')
    {
        let types_str = &inner[bracket_start + 1..bracket_start + bracket_end];
        task_types = types_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    Phase {
        engagement,
        task_types,
        description,
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
    fn test_extract_surprises() {
        let text = "**Surprise** — The correlation was negative, not positive as all previous sessions assumed.\n**Spark** — something";
        let surprises = extract_surprises(text);
        assert_eq!(surprises.len(), 1);
        assert!(surprises[0].contains("correlation was negative"));
    }

    #[test]
    fn test_extract_surprises_plain() {
        let text = "Some context.\n\nSurprise: The refactored code was slower, not faster.";
        let surprises = extract_surprises(text);
        assert_eq!(surprises.len(), 1);
        assert!(surprises[0].contains("slower"));
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
    fn test_extract_phases_structured() {
        let text = "**Phase** (engagement: 4, [debugging, design]) — Process forensics and tracing.\n\n**Phase** (engagement: 1, [babysitting]) — Download monitoring.";
        let phases = extract_phases(text);
        assert_eq!(phases.len(), 2);
        let p1 = parse_phase(&phases[0]);
        assert_eq!(p1.engagement, Some(4.0));
        assert_eq!(p1.task_types, vec!["debugging", "design"]);
        assert!(p1.description.starts_with("Process forensics"));
        let p2 = parse_phase(&phases[1]);
        assert_eq!(p2.engagement, Some(1.0));
        assert_eq!(p2.task_types, vec!["babysitting"]);
    }

    #[test]
    fn test_extract_phases_unstructured() {
        let text = "**Phase** — Just a description without structured data.";
        let phases = extract_phases(text);
        assert_eq!(phases.len(), 1);
        let p = parse_phase(&phases[0]);
        assert!(p.engagement.is_none());
        assert!(p.task_types.is_empty());
        assert!(p.description.starts_with("Just a description"));
    }

    #[test]
    fn test_extract_phases_engagement_only() {
        let text = "**Phase** (engagement: 5) — Pure design work.";
        let phases = extract_phases(text);
        let p = parse_phase(&phases[0]);
        assert_eq!(p.engagement, Some(5.0));
        assert!(p.task_types.is_empty());
        assert!(p.description.starts_with("Pure design"));
    }

    #[test]
    fn test_extract_phases_plain_format() {
        let text = "Some prose.\n\nPhase: Quick debugging sprint.";
        let phases = extract_phases(text);
        assert_eq!(phases.len(), 1);
        let p = parse_phase(&phases[0]);
        assert!(p.description.starts_with("Quick debugging"));
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
            "unclassified"
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
    // --- marker shapes (#22) ---

    #[test]
    fn bold_span_covering_the_whole_sentence_is_extracted() {
        // The shape the 2026-08-05 minimermetic note used, which was dropped.
        let text = "**Opinion: three silences were three chances to ask the right question, \
            and I took the third.** The discipline follows.";
        assert_eq!(
            extract_opinions(text),
            vec![
                "three silences were three chances to ask the right question, and I took the \
                 third. The discipline follows."
            ]
        );
    }

    #[test]
    fn the_shapes_that_already_worked_give_the_same_entries() {
        for text in [
            "**Opinion** \u{2014} worth keeping.",
            "**Opinion**: worth keeping.",
            "**Opinion:** worth keeping.",
            "Opinion: worth keeping.",
            "intro line\nOpinion: worth keeping.",
        ] {
            assert_eq!(extract_opinions(text), vec!["worth keeping."], "{text}");
        }
    }

    #[test]
    fn qualified_markers_keep_their_qualifier_and_text() {
        assert_eq!(
            extract_surprises("**Surprise (the good one)** \u{2014} it held."),
            vec!["(the good one) \u{2014} it held."]
        );
        assert_eq!(
            extract_surprises("**Surprise, bigger** \u{2014} nothing was signed."),
            vec!["bigger \u{2014} nothing was signed."]
        );
        assert_eq!(
            extract_surprises("**Surprise #2:** the cache was cold."),
            vec!["#2: the cache was cold."]
        );
        assert_eq!(
            extract_surprises("**Surprise \u{2014} the README was a failed read.**"),
            vec!["the README was a failed read."]
        );
    }

    #[test]
    fn bold_text_that_only_starts_with_a_marker_word_is_not_an_entry() {
        assert!(extract_surprises("**Surprises** were few.").is_empty());
        assert!(extract_changes("**Change log** for the release.").is_empty());
        assert!(extract_phases("**Phase 1** went fine.").is_empty());
        assert!(extract_frictions("**Friction patterns:** stuck loop (3x)").is_empty());
        assert!(extract_opinions("**Opinionated** tooling.").is_empty());
        assert!(!has_marker("**Surprises** were few.", "Surprise"));
    }

    #[test]
    fn one_note_mixing_shapes_yields_each_entry_once() {
        let text = "**Opinion** \u{2014} first.\n\n**Opinion: second.**\n\n\
            **Opinion:** third.\n\n**Opinion** \u{2014} first.";
        assert_eq!(extract_opinions(text), vec!["first.", "second.", "third."]);
    }

    #[test]
    fn has_marker_sees_every_shape() {
        for text in [
            "**Surprise** \u{2014} x",
            "**Surprise:** x",
            "**Surprise: x**",
            "**Surprise, the good kind** x",
        ] {
            assert!(has_marker(text, "Surprise"), "{text}");
        }
        assert!(!has_marker("Surprise: plain start-of-line", "Surprise"));
        assert!(!has_marker("no markers here", "Surprise"));
    }

    #[test]
    fn a_marker_at_the_very_end_of_the_text_does_not_panic() {
        assert!(extract_sparks("trailing **Spark").is_empty());
        assert!(extract_sparks("trailing **Spark**").is_empty());
        assert!(extract_sparks("**Spark: **").is_empty());
    }
}
