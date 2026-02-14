use crate::hook_io;
use serde_json::Value;
use std::fmt::Write;

pub fn run() {
    let input = hook_io::raw_stdin();

    // Parse fields from input
    let model = input
        .pointer("/model/display_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Claude")
        .replace("Claude ", "");

    let current_dir = input
        .pointer("/workspace/current_dir")
        .and_then(|v| v.as_str())
        .or_else(|| std::env::var("PWD").ok().as_deref().map(|_| ""))
        .unwrap_or("");
    let current_dir = if current_dir.is_empty() {
        std::env::var("PWD").unwrap_or_else(|_| ".".into())
    } else {
        current_dir.to_string()
    };

    let lines_added = get_u64(&input, "/cost/total_lines_added");
    let lines_removed = get_u64(&input, "/cost/total_lines_removed");
    let session_id = input
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Git information
    let (repo_name, branch, git_status) = get_git_info(&current_dir);

    // Exo-self indicator
    let exo_indicator = if !session_id.is_empty() {
        let home = std::env::var("HOME").unwrap_or_default();
        let state_path = format!("{home}/.claude/exo-self/sessions/state-{session_id}.json");
        if std::path::Path::new(&state_path).exists() {
            "\x1b[1;35m◈\x1b[0m "
        } else {
            ""
        }
    } else {
        ""
    };

    // Build Line 1: Exo + Model + Repo:Branch + Status + Changes
    let mut line1 = format!("{exo_indicator}\x1b[1;36m[{model}]\x1b[0m ");

    if !repo_name.is_empty() {
        write!(line1, "\x1b[1;32m{repo_name}\x1b[0m").unwrap();
        if !branch.is_empty() {
            write!(line1, ":\x1b[1;34m{branch}\x1b[0m").unwrap();
        }
    }

    if !git_status.is_empty() {
        write!(line1, " \x1b[1;31m{git_status}\x1b[0m").unwrap();
    }

    if lines_added > 0 || lines_removed > 0 {
        write!(
            line1,
            " | \x1b[0;32m+{lines_added}\x1b[0m/\x1b[0;31m-{lines_removed}\x1b[0m"
        )
        .unwrap();
    }

    // Build Line 2: Context bar + percentage + duration + cost
    let duration_ms = get_u64(&input, "/cost/total_duration_ms");
    let duration_hours = duration_ms / 3_600_000;
    let duration_min = (duration_ms % 3_600_000) / 60_000;

    let cost_usd = input
        .pointer("/cost/total_cost_usd")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let total_tokens = input
        .pointer("/context_window/context_window_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(200_000);

    let (used_tokens, usage_pct) = compute_usage(&input, total_tokens);
    let free_tokens = total_tokens.saturating_sub(used_tokens);

    // Generate brick visualization (20 bricks)
    let total_bricks: u64 = 20;
    let used_bricks = if total_tokens > 0 {
        (used_tokens * total_bricks / total_tokens).min(total_bricks)
    } else {
        0
    };
    let free_bricks = total_bricks - used_bricks;

    let mut line2 = String::from("[");
    for _ in 0..used_bricks {
        line2.push_str("\x1b[0;36m■\x1b[0m");
    }
    for _ in 0..free_bricks {
        line2.push_str("\x1b[2;37m□\x1b[0m");
    }
    write!(line2, "] \x1b[1m{usage_pct}%\x1b[0m").unwrap();
    write!(line2, " | {duration_hours}h{duration_min}m").unwrap();

    if cost_usd > 0.0 {
        write!(line2, " | \x1b[0;33m${cost_usd:.2}\x1b[0m").unwrap();
    }

    // Write context data to shared file for exo-self hooks
    write_context_window_json(session_id, used_tokens, free_tokens, usage_pct, &input);

    // Output
    println!("{line1}");
    print!("{line2}");
}

fn get_u64(input: &Value, pointer: &str) -> u64 {
    input
        .pointer(pointer)
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
        .unwrap_or(0)
}

fn compute_usage(input: &Value, total_tokens: u64) -> (u64, u64) {
    // Try new percentage fields first (Claude Code 2.1.6+)
    if let Some(used_pct) = input
        .pointer("/context_window/used_percentage")
        .and_then(|v| v.as_f64())
    {
        let usage_pct = used_pct as u64;
        let used_tokens = total_tokens * usage_pct / 100;
        return (used_tokens, usage_pct);
    }

    // Fallback: calculate from current_usage
    if let Some(current_usage) = input.pointer("/context_window/current_usage") {
        let input_tokens = current_usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_creation = current_usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = current_usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let used_tokens = input_tokens + cache_creation + cache_read;
        let usage_pct = if total_tokens > 0 {
            used_tokens * 100 / total_tokens
        } else {
            0
        };
        return (used_tokens, usage_pct);
    }

    (0, 0)
}

fn get_git_info(dir: &str) -> (String, String, String) {
    let run = |args: &[&str]| -> String {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    };

    // Check if in git repo
    let git_dir = run(&["rev-parse", "--git-dir"]);
    if git_dir.is_empty() {
        return (String::new(), String::new(), String::new());
    }

    let toplevel = run(&["rev-parse", "--show-toplevel"]);
    let repo_name = std::path::Path::new(&toplevel)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let branch = run(&["branch", "--show-current"]);
    let branch = if branch.is_empty() {
        "detached".into()
    } else {
        branch
    };

    // Git status indicators
    let mut status = String::new();
    let porcelain = run(&["status", "--porcelain"]);
    if !porcelain.is_empty() {
        status.push('*');
    }

    // Ahead/behind
    let upstream = run(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
    if !upstream.is_empty() {
        let ahead = run(&["rev-list", "--count", &format!("{upstream}..HEAD")]);
        let behind = run(&["rev-list", "--count", &format!("HEAD..{upstream}")]);
        if let Ok(n) = ahead.parse::<u64>()
            && n > 0
        {
            write!(status, "↑{n}").unwrap();
        }
        if let Ok(n) = behind.parse::<u64>()
            && n > 0
        {
            write!(status, "↓{n}").unwrap();
        }
    }

    (repo_name, branch, status)
}

fn write_context_window_json(
    session_id: &str,
    used_tokens: u64,
    free_tokens: u64,
    usage_pct: u64,
    input: &Value,
) {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{home}/.claude/exo-self/.context-window.json");

    let used_percentage = input
        .pointer("/context_window/used_percentage")
        .and_then(|v| v.as_f64());
    let remaining_percentage = input
        .pointer("/context_window/remaining_percentage")
        .and_then(|v| v.as_f64());
    let context_window_size = input
        .pointer("/context_window/context_window_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(200_000);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let json = serde_json::json!({
        "used_percentage": used_percentage,
        "remaining_percentage": remaining_percentage,
        "context_window_size": context_window_size,
        "exceeds_200k_tokens": input.get("exceeds_200k_tokens").and_then(|v| v.as_bool()).unwrap_or(false),
        "used_tokens": used_tokens,
        "free_tokens": free_tokens,
        "usage_pct": usage_pct,
        "session_id": session_id,
        "updated_at": now
    });

    let _ = std::fs::write(path, serde_json::to_string(&json).unwrap_or_default());
}
