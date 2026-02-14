use crate::hook_io::{self, HookInput};
use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::state::{self, SessionState};

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();
    let _ = std::fs::create_dir_all(&paths.handoffs_dir);

    let session_id = &input.session_id;

    // Step 1: Automatic handoff extraction from transcript
    if !input.transcript_path.is_empty() && std::fs::metadata(&input.transcript_path).is_ok() {
        let handoff_name = if session_id.is_empty() {
            "latest".to_string()
        } else {
            session_id.clone()
        };
        let handoff_file = paths.handoff_file(&handoff_name);

        // Run extract-handoff inline (it's a pure function)
        let content = extract_handoff_content(&input.transcript_path);
        if !content.is_empty() {
            let _ = std::fs::write(&handoff_file, &content);
            // Also save as "latest"
            let _ = std::fs::write(paths.handoffs_dir.join("latest.md"), &content);
        }
    }

    // Step 2: Update state and meta
    let mut state = SessionState::load(&paths, session_id);

    state.compactions += 1;
    state.last_compaction = state::now();
    state.last_compaction_trigger = input.trigger.clone();

    state.save_with_shared(&paths);

    // Update meta
    let mut meta = Meta::load(&paths.meta);
    meta.total_compactions += 1;
    meta.last_compaction = Some(
        chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%.6f")
            .to_string(),
    );
    meta.save(&paths.meta);

    let compaction_num = state.compactions;
    let trigger = if input.trigger.is_empty() {
        "unknown"
    } else {
        &input.trigger
    };

    let msg = format!(
        "## Exo-Self: Pre-Compaction (#{compaction_num}, {trigger})\n\n\
        Session handoff has been **automatically saved** to `~/.claude/exo-self/handoffs/`.\n\n\
        If you have subjective observations worth preserving (how the work felt, patterns you noticed, \
        things that surprised you), write them to `journal.md` now. Otherwise, carry on — your next \
        instance will have the factual context."
    );

    hook_io::hook_output("PreCompact", &msg);
}

/// Inline handoff extraction (same logic as extract_handoff::run but returns String)
fn extract_handoff_content(transcript_path: &str) -> String {
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::io::BufRead;

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
                        let t = if text.len() > 200 { &text[..197] } else { text };
                        user_prompts.push(t.to_string());
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
                                let t = if text.len() > 200 { &text[..197] } else { text };
                                user_prompts.push(t.to_string());
                            }
                        }
                    }
                }
            }
            "assistant" => {
                if let Some(arr) = content.as_array() {
                    for block in arr {
                        let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if btype == "text" {
                            let text = block
                                .get("text")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .trim();
                            if !text.is_empty() {
                                assistant_texts.push(text.to_string());
                            }
                        } else if btype == "tool_use" {
                            let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            if !tool.is_empty() {
                                tools_used.insert(tool.to_string());
                            }
                            if (tool == "Edit" || tool == "Write")
                                && let Some(fp) = block
                                    .get("input")
                                    .and_then(|i| i.get("file_path"))
                                    .and_then(|p| p.as_str())
                            {
                                files_modified.insert(fp.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut sections = Vec::new();
    if !user_prompts.is_empty() {
        let first: Vec<_> = user_prompts
            .iter()
            .take(3)
            .map(|p| format!("- {p}"))
            .collect();
        let mut s = first.join("\n");
        if user_prompts.len() > 3 {
            let last: Vec<_> = user_prompts
                .iter()
                .rev()
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|p| format!("- {p}"))
                .collect();
            s.push_str("\n...\n");
            s.push_str(&last.join("\n"));
        }
        sections.push(format!("## User Requests\n\n{s}"));
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
        sections.push(format!(
            "## Tools Used\n\n{}",
            tools_used.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if let Some(last) = assistant_texts.last() {
        let mut summary = last.clone();
        if summary.len() > 800 {
            summary.truncate(800);
            summary.push_str("...");
        }
        sections.push(format!("## Last Response Summary\n\n{summary}"));
    }

    let mut result = sections.join("\n\n");
    if result.len() > 3000 {
        result.truncate(2997);
        result.push_str("...");
    }
    result
}
