use serde_json::Value;
use std::collections::BTreeSet;
use std::io::BufRead;

/// Extract an experiential handoff summary from a Claude Code transcript JSONL.
/// Prioritizes continuity-relevant content: working direction, discoveries,
/// unfinished threads. Structural data (files, tools) fills remaining budget.
pub fn run(transcript_path: &str) {
    let result = extract(transcript_path, 3000);
    print!("{result}");
}

pub fn extract(transcript_path: &str, max_chars: usize) -> String {
    let file = match std::fs::File::open(transcript_path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };

    let reader = std::io::BufReader::new(file);
    let mut user_prompts = Vec::new();
    let mut assistant_texts = Vec::new();
    let mut files_modified = BTreeSet::new();

    // Experiential signal collectors
    let mut insights = Vec::new();
    let mut markers = Vec::new(); // Spark, Friction, Change
    let mut hypotheses = Vec::new();
    let mut unfinished = Vec::new();

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let Ok(obj) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        let Some(msg) = obj.get("message") else {
            continue;
        };
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
        let content = &msg["content"];

        match role {
            "user" => {
                if let Some(text) = content.as_str() {
                    let text = text.trim();
                    if is_user_text(text) {
                        user_prompts.push(truncate(text, 200));
                    }
                } else if let Some(arr) = content.as_array() {
                    for block in arr {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            let text = block
                                .get("text")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .trim();
                            if is_user_text(text) {
                                user_prompts.push(truncate(text, 200));
                            }
                        }
                    }
                }
            }
            "assistant" => {
                if let Some(arr) = content.as_array() {
                    for block in arr {
                        let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match btype {
                            "text" => {
                                let text = block
                                    .get("text")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("")
                                    .trim();
                                if !text.is_empty() {
                                    extract_signals(
                                        text,
                                        &mut insights,
                                        &mut markers,
                                        &mut hypotheses,
                                        &mut unfinished,
                                    );
                                    assistant_texts.push(text.to_string());
                                }
                            }
                            "tool_use" => {
                                let tool_name =
                                    block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                if (tool_name == "Edit" || tool_name == "Write")
                                    && let Some(fp) = block
                                        .get("input")
                                        .and_then(|i| i.get("file_path"))
                                        .and_then(|p| p.as_str())
                                {
                                    files_modified.insert(fp.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // --- Build handoff: experiential first, structural after ---
    let mut sections = Vec::new();

    // 1. Working direction — last substantive assistant text
    let direction = last_substantive_text(&assistant_texts, 600);
    if !direction.is_empty() {
        sections.push(format!("## Working Direction\n\n{direction}"));
    }

    // 2. Discoveries — insights and spark markers
    let mut discoveries = Vec::new();
    for insight in insights.iter().take(3) {
        // Skip delimiter lines that leaked through
        if insight.contains("───") {
            continue;
        }
        discoveries.push(format!("- {insight}"));
    }
    for marker in markers.iter().filter(|m| m.starts_with("Spark")) {
        discoveries.push(format!("- {marker}"));
    }
    if !discoveries.is_empty() {
        sections.push(format!("## Discoveries\n\n{}", discoveries.join("\n")));
    }

    // 3. Friction — friction and change markers
    let mut friction_items = Vec::new();
    for marker in &markers {
        if marker.starts_with("Friction") || marker.starts_with("Change") {
            friction_items.push(format!("- {marker}"));
        }
    }
    if !friction_items.is_empty() {
        sections.push(format!(
            "## Friction & Lessons\n\n{}",
            friction_items.join("\n")
        ));
    }

    // 4. Unfinished threads
    if !unfinished.is_empty() {
        let items: Vec<_> = unfinished
            .iter()
            .take(5)
            .map(|t| format!("- {t}"))
            .collect();
        sections.push(format!("## Unfinished Threads\n\n{}", items.join("\n")));
    }

    // 5. Hypotheses — working theories not yet confirmed
    if !hypotheses.is_empty() {
        let items: Vec<_> = hypotheses
            .iter()
            .take(3)
            .map(|h| format!("- {h}"))
            .collect();
        sections.push(format!("## Working Hypotheses\n\n{}", items.join("\n")));
    }

    // 6. User requests (compact — first 2 + last 1)
    if !user_prompts.is_empty() {
        let mut prompt_lines: Vec<String> = user_prompts
            .iter()
            .take(2)
            .map(|p| format!("- {p}"))
            .collect();
        if user_prompts.len() > 2 {
            if user_prompts.len() > 3 {
                prompt_lines.push(format!("- ...({} more)", user_prompts.len() - 3));
            }
            prompt_lines.push(format!("- {}", user_prompts.last().unwrap()));
        }
        sections.push(format!("## User Requests\n\n{}", prompt_lines.join("\n")));
    }

    // 7. Files modified (compact — max 10, grouped hint)
    if !files_modified.is_empty() {
        let count = files_modified.len();
        let list: Vec<_> = files_modified
            .iter()
            .take(10)
            .map(|f| format!("- {f}"))
            .collect();
        let mut section = list.join("\n");
        if count > 10 {
            section.push_str(&format!("\n- ...and {} more", count - 10));
        }
        sections.push(format!("## Files Modified ({count})\n\n{section}"));
    }

    let mut result = sections.join("\n\n");
    if result.len() > max_chars {
        // Truncate from the end (structural sections get cut first)
        let end = crate::markdown::safe_truncate(&result, max_chars.saturating_sub(3));
        result.truncate(end);
        result.push_str("...");
    }
    result
}

/// Extract experiential signals from an assistant text block.
fn extract_signals(
    text: &str,
    insights: &mut Vec<String>,
    markers: &mut Vec<String>,
    hypotheses: &mut Vec<String>,
    unfinished: &mut Vec<String>,
) {
    for line in text.lines() {
        let trimmed = line.trim();

        // Insight blocks: capture content between ★ delimiters
        if trimmed.contains("★ Insight") {
            // The insight title line itself — skip, content follows
            continue;
        }

        // Exo-self markers: **Spark**, **Friction**, **Change**
        for prefix in ["**Spark**", "**Friction**", "**Change**"] {
            if trimmed.starts_with(prefix) {
                let label = &prefix[2..prefix.len() - 2]; // strip **
                let rest = trimmed[prefix.len()..].trim_start_matches([' ', '—', ':', '-']);
                if !rest.is_empty() {
                    markers.push(format!("{label}: {}", truncate(rest.trim(), 150)));
                }
            }
        }

        // Unfinished work signals — only from lines that look like action items
        // (start with bullet, "TODO", "Next", or contain strong intent signals)
        let lower = trimmed.to_lowercase();
        let looks_actionable = trimmed.starts_with('-')
            || trimmed.starts_with('*')
            || trimmed.starts_with("TODO")
            || lower.starts_with("next")
            || lower.starts_with("still need")
            || lower.starts_with("remaining");
        if looks_actionable {
            for pattern in [
                "still need",
                "haven't yet",
                "todo",
                "next step",
                "remaining",
                "left to do",
                "not yet implemented",
            ] {
                if lower.contains(pattern) && trimmed.len() > 20 {
                    unfinished.push(truncate(trimmed, 150));
                    break;
                }
            }
        }
    }

    // Hypotheses: sentences containing hypothesis-like patterns
    // Scan the full text for these (not line-by-line, since they can span formatting)
    for sentence in split_sentences(text) {
        let lower = sentence.to_lowercase();
        let is_hypothesis = ["i think the", "the issue is", "the problem is", "turns out"]
            .iter()
            .any(|p| lower.contains(p));
        if is_hypothesis && sentence.len() > 30 {
            hypotheses.push(truncate(&sentence, 150));
        }
    }

    // Insight content: look for lines between the ★ delimiters that aren't the delimiter itself
    let mut in_insight = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("★ Insight") {
            in_insight = true;
            continue;
        }
        if in_insight && trimmed.starts_with("───") {
            if insights.last().is_some() {
                // Closing delimiter — end the insight
                in_insight = false;
            }
            continue;
        }
        if in_insight && !trimmed.is_empty() {
            insights.push(truncate(trimmed, 200));
        }
    }
}

/// Find the last substantive assistant text (>min_len chars, not trivial).
fn last_substantive_text(texts: &[String], max_chars: usize) -> String {
    let min_len = 80;

    // Walk backwards to find the last substantive block
    for text in texts.iter().rev() {
        if text.len() >= min_len {
            // Skip texts that are purely structural (just file listings, etc.)
            let lower = text.to_lowercase();
            if lower.starts_with("here are the") || lower.starts_with("the files") {
                continue;
            }
            return truncate(text, max_chars);
        }
    }
    // Fallback to absolute last if nothing substantive
    texts
        .last()
        .map(|t| truncate(t, max_chars))
        .unwrap_or_default()
}

/// Rough sentence splitter — splits on ". " followed by uppercase, or on newlines.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if ch == '\n' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }
    sentences
}

/// Filter out system-injected content from user text blocks.
fn is_user_text(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with("<system-reminder>")
        && !text.starts_with("<local-command-caveat>")
        && !text.starts_with("<command-name>")
        && !text.starts_with("<command-message>")
        && !text.starts_with("<command-args>")
        && !text.starts_with("<local-command-stdout>")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let end = crate::markdown::safe_truncate(s, max.saturating_sub(3));
        format!("{}...", &s[..end])
    }
}
