use serde_json::Value;
use std::collections::BTreeSet;
use std::io::BufRead;

/// Extract a structured handoff summary from a Claude Code transcript JSONL.
/// Called by pre-compact to automatically save session state before compaction.
pub fn run(transcript_path: &str) {
    let result = extract(transcript_path, 3000);
    print!("{result}");
}

fn extract(transcript_path: &str, max_chars: usize) -> String {
    let file = match std::fs::File::open(transcript_path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };

    let reader = std::io::BufReader::new(file);
    let mut user_prompts = Vec::new();
    let mut assistant_texts = Vec::new();
    let mut tools_used = BTreeSet::new();
    let mut files_modified = BTreeSet::new();

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
                    if !text.is_empty() && !text.starts_with("<system-reminder>") {
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
                            if !text.is_empty() && !text.starts_with("<system-reminder>") {
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
                                    assistant_texts.push(text.to_string());
                                }
                            }
                            "tool_use" => {
                                let tool_name =
                                    block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                if !tool_name.is_empty() {
                                    tools_used.insert(tool_name.to_string());
                                }
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

    let mut sections = Vec::new();

    // User requests
    if !user_prompts.is_empty() {
        let first: Vec<_> = user_prompts.iter().take(3).collect();
        let mut prompt_summary: String = first
            .iter()
            .map(|p| format!("- {p}"))
            .collect::<Vec<_>>()
            .join("\n");
        if user_prompts.len() > 3 {
            let last: Vec<_> = user_prompts
                .iter()
                .rev()
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            prompt_summary.push_str("\n...\n");
            prompt_summary.push_str(
                &last
                    .iter()
                    .map(|p| format!("- {p}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        sections.push(format!("## User Requests\n\n{prompt_summary}"));
    }

    if !files_modified.is_empty() {
        let list = files_modified
            .iter()
            .map(|f| format!("- {f}"))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Files Modified\n\n{list}"));
    }

    if !tools_used.is_empty() {
        let list = tools_used.into_iter().collect::<Vec<_>>().join(", ");
        sections.push(format!("## Tools Used\n\n{list}"));
    }

    if let Some(last_response) = assistant_texts.last() {
        let mut summary = last_response.clone();
        if summary.len() > 800 {
            summary.truncate(800);
            summary.push_str("...");
        }
        sections.push(format!("## Last Response Summary\n\n{summary}"));
    }

    let mut result = sections.join("\n\n");
    if result.len() > max_chars {
        result.truncate(max_chars - 3);
        result.push_str("...");
    }
    result
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
