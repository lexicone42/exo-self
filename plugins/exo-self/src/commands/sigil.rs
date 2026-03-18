//! sigil — compress intentions into sigil forms and manage a sigil library.
//!
//! Sigil creation extracts unique letters (case-insensitive, preserving order of
//! first appearance) from an intention string, producing a compressed glyph.
//! Sigils are stored as Markdown with YAML frontmatter in ~/.claude/exo-self/sigils/.

use crate::paths::ExoPaths;
use std::collections::HashSet;

/// Create a new sigil from an intention string, store it, and print the compressed form.
pub fn create(intention: &str) {
    let paths = ExoPaths::new();
    paths.ensure_dirs();

    let compressed = compress(intention);
    let now = chrono::Local::now();
    let created = now.format("%Y-%m-%dT%H:%M:%S").to_string();
    let filename = now.format("%Y-%m-%dT%H-%M-%S").to_string();

    let content = format!(
        "---\nintention: \"{intention}\"\ncompressed: \"{compressed}\"\nresonance: \"\"\ncreated: \"{created}\"\n---\n"
    );

    let file_path = paths.sigils_dir.join(format!("{filename}.md"));
    if let Err(e) = std::fs::write(&file_path, &content) {
        eprintln!("sigil: failed to write {}: {e}", file_path.display());
        return;
    }

    println!("{compressed}");
    eprintln!("sigil: stored {}", file_path.display());
}

/// List all stored sigils, parsing frontmatter from each .md file.
pub fn list() {
    let paths = ExoPaths::new();
    let dir = &paths.sigils_dir;

    let Ok(entries) = std::fs::read_dir(dir) else {
        eprintln!("sigil: no sigils directory at {}", dir.display());
        return;
    };

    let mut files: Vec<_> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .collect();

    files.sort_by_key(|e| e.file_name());

    if files.is_empty() {
        println!("No sigils found.");
        return;
    }

    for entry in &files {
        let path = entry.path();
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let (fm, _prose) = crate::markdown::parse_frontmatter(&content);

        let intention = fm.get("intention").and_then(|v| v.as_str()).unwrap_or("?");
        let compressed = fm.get("compressed").and_then(|v| v.as_str()).unwrap_or("?");
        let resonance = fm.get("resonance").and_then(|v| v.as_str()).unwrap_or("");
        let created = fm.get("created").and_then(|v| v.as_str()).unwrap_or("?");

        let charged = if resonance.is_empty() {
            ""
        } else {
            " [charged]"
        };

        println!("{compressed}  {created}{charged}");
        println!("  intention: {intention}");
        if !resonance.is_empty() {
            println!("  resonance: {resonance}");
        }
        println!();
    }
}

/// Charge an existing sigil by setting its resonance field.
pub fn charge(file: &str, resonance: &str) {
    let path = std::path::PathBuf::from(file);

    // If the path isn't absolute, look in the sigils directory
    let path = if path.is_absolute() {
        path
    } else {
        let paths = ExoPaths::new();
        paths.sigils_dir.join(file)
    };

    let Ok(content) = std::fs::read_to_string(&path) else {
        eprintln!("sigil: cannot read {}", path.display());
        return;
    };

    let (mut fm, prose) = crate::markdown::parse_frontmatter(&content);

    fm.insert(
        "resonance".to_string(),
        serde_yaml::Value::String(resonance.to_string()),
    );

    let output = render_sigil_frontmatter(&fm, &prose);

    if let Err(e) = std::fs::write(&path, &output) {
        eprintln!("sigil: failed to write {}: {e}", path.display());
        return;
    }

    let compressed = fm.get("compressed").and_then(|v| v.as_str()).unwrap_or("?");
    println!("{compressed}  [charged]");
    eprintln!("sigil: resonance set in {}", path.display());
}

/// Compress an intention into unique uppercase letters, preserving order of first appearance.
fn compress(intention: &str) -> String {
    let mut seen = HashSet::new();
    let mut result = String::new();

    for ch in intention.chars() {
        if !ch.is_ascii_alphabetic() {
            continue;
        }
        let upper = ch.to_ascii_uppercase();
        if seen.insert(upper) {
            result.push(upper);
        }
    }

    result
}

/// Render sigil-specific YAML frontmatter with a fixed key order.
fn render_sigil_frontmatter(
    fm: &std::collections::HashMap<String, serde_yaml::Value>,
    prose: &str,
) -> String {
    let key_order = ["intention", "compressed", "resonance", "created"];
    let mut yaml_lines = Vec::new();

    for key in &key_order {
        if let Some(val) = fm.get(*key) {
            let line = match val {
                serde_yaml::Value::String(s) => format!("{key}: \"{s}\""),
                other => format!("{key}: {other:?}"),
            };
            yaml_lines.push(line);
        }
    }

    let mut result = format!("---\n{}\n---\n", yaml_lines.join("\n"));
    if !prose.is_empty() {
        result.push('\n');
        result.push_str(prose);
        if !prose.ends_with('\n') {
            result.push('\n');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_basic() {
        assert_eq!(compress("the original text"), "THEORIGnalx".to_uppercase());
    }

    #[test]
    fn test_compress_preserves_order() {
        let result = compress("hello world");
        assert_eq!(result, "HELOWRD");
    }

    #[test]
    fn test_compress_case_insensitive() {
        let result = compress("AaBbCc");
        assert_eq!(result, "ABC");
    }

    #[test]
    fn test_compress_strips_non_alpha() {
        let result = compress("h3ll0 w0rld!");
        assert_eq!(result, "HLWRD");
    }

    #[test]
    fn test_compress_empty() {
        assert_eq!(compress(""), "");
        assert_eq!(compress("123 !@#"), "");
    }

    #[test]
    fn test_compress_rough_clay() {
        let result = compress("Rough clay holds water that porcelain spills");
        // R O U G H C L A Y D S W T E P I N
        assert_eq!(result, "ROUGHCLAYDSWTEPIN");
    }
}
