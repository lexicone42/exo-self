use crate::config::Config;
use crate::hook_io::{self, HookInput};
use crate::paths::ExoPaths;
use crate::state::{self, SessionState};

pub fn run() {
    let input = HookInput::from_stdin();
    if input.session_id.is_empty() {
        hook_io::empty_output();
        return;
    }

    let paths = ExoPaths::new();
    let cfg = Config::load(&paths.config);
    let mut state = SessionState::load(&paths, &input.session_id);

    // Increment failure count
    state.tool_failures += 1;
    state.last_failure_at = state::now();

    // Track which tools are failing
    let tool_name = if input.tool_name.is_empty() {
        "unknown".to_string()
    } else {
        input.tool_name.clone()
    };
    *state.failure_tools.entry(tool_name.clone()).or_insert(0) += 1;

    // Track consecutive same-tool failures (stuck loop detection)
    if tool_name == state.last_failure_tool {
        state.consecutive_same_tool += 1;
    } else {
        state.consecutive_same_tool = 1;
        state.last_failure_tool = tool_name.clone();
    }

    // Classify the friction cause from tool_input + error
    let category = classify_failure(&input);
    *state.failure_categories.entry(category).or_insert(0) += 1;

    // Nudge once when threshold is crossed
    if state.tool_failures == cfg.failure_nudge_threshold && !state.failure_nudge_sent {
        state.failure_nudge_sent = true;

        let (top_tool, top_count) = state
            .failure_tools
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(tool, count)| (tool.as_str(), *count))
            .unwrap_or(("tools", 0));

        let top_category = state
            .failure_categories
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(cat, _)| cat.as_str())
            .unwrap_or("unknown");

        let stuck_note = if state.consecutive_same_tool >= 3 {
            format!(
                " {} has failed {} times consecutively — you may be stuck.",
                state.last_failure_tool, state.consecutive_same_tool
            )
        } else {
            String::new()
        };

        let msg = format!(
            "Exo-self: {} tool failures ({}: {}x, category: {}).{} Worth noting in your session notes if it's causing friction.",
            state.tool_failures, top_tool, top_count, top_category, stuck_note
        );

        state.save(&paths);
        hook_io::hook_output("PostToolUseFailure", &msg);
    } else {
        state.save(&paths);
        hook_io::empty_output();
    }
}

/// Classify a tool failure into a friction category based on tool_input and error.
fn classify_failure(input: &HookInput) -> String {
    let error_lower = input.error.to_lowercase();
    let tool = input.tool_name.as_str();

    // Extract command string for Bash tools
    let command = input
        .tool_input
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let cmd_lower = command.to_lowercase();

    // Permission / sandbox issues (any tool)
    if error_lower.contains("permission denied")
        || error_lower.contains("eacces")
        || error_lower.contains("sandbox")
        || error_lower.contains("denied this tool")
    {
        return "permissions".into();
    }

    // Edit-specific: string not found or not unique
    if tool == "Edit" {
        if error_lower.contains("not unique") || error_lower.contains("multiple occurrences") {
            return "edit_ambiguous".into();
        }
        if error_lower.contains("not found") || error_lower.contains("does not contain") {
            return "edit_stale".into();
        }
    }

    // Bash-specific classification by command content
    if tool == "Bash" && !cmd_lower.is_empty() {
        // Pre-commit / lint / format
        if cmd_lower.contains("pre-commit")
            || cmd_lower.contains("clippy")
            || cmd_lower.contains("ruff")
            || cmd_lower.contains("eslint")
            || cmd_lower.contains("cargo fmt")
            || cmd_lower.contains("prettier")
        {
            return "pre_commit".into();
        }

        // Test execution
        if cmd_lower.contains("cargo test")
            || cmd_lower.contains("pytest")
            || cmd_lower.contains("npm test")
            || cmd_lower.contains("jest")
            || cmd_lower.contains("vitest")
        {
            return "test_iteration".into();
        }

        // Build / compilation
        if cmd_lower.contains("cargo build")
            || cmd_lower.contains("cargo check")
            || cmd_lower.contains("npm run build")
            || cmd_lower.contains("make")
            || cmd_lower.contains("gcc")
            || cmd_lower.contains("rustc")
        {
            return "build_failure".into();
        }

        // Infrastructure / deployment
        if cmd_lower.contains("cdk")
            || cmd_lower.contains("cloudformation")
            || cmd_lower.contains("terraform")
            || cmd_lower.contains("kubectl")
            || cmd_lower.contains("docker")
            || cmd_lower.contains("aws ")
            || cmd_lower.contains("gcloud")
        {
            return "infrastructure".into();
        }

        // Git operations
        if cmd_lower.starts_with("git ") {
            return "git_operation".into();
        }
    }

    // Type/compile errors in the error message (any tool)
    if error_lower.contains("type mismatch")
        || error_lower.contains("e0308")
        || error_lower.contains("expected type")
        || error_lower.contains("cannot find type")
    {
        return "type_system".into();
    }

    // File not found (Read, Write, etc.)
    if error_lower.contains("no such file")
        || error_lower.contains("file not found")
        || error_lower.contains("does not exist")
    {
        return "file_not_found".into();
    }

    // Network / API errors
    if error_lower.contains("connection refused")
        || error_lower.contains("timeout")
        || error_lower.contains("timed out")
        || error_lower.contains("dns")
        || error_lower.contains("could not resolve")
        || error_lower.contains("ssl")
        || error_lower.contains("certificate")
    {
        return "network".into();
    }

    // Cargo / Rust specific (not caught by build_failure above)
    if error_lower.contains("cargo")
        || error_lower.contains("rustc")
        || error_lower.contains("unresolved")
        || error_lower.contains("e0") && error_lower.len() > 4
    {
        return "build_failure".into();
    }

    // Lock contention / resource busy
    if error_lower.contains("lock")
        || error_lower.contains("text file busy")
        || error_lower.contains("resource busy")
        || error_lower.contains("blocking")
    {
        return "lock_contention".into();
    }

    // Syntax / parse errors
    if error_lower.contains("syntax error")
        || error_lower.contains("parse error")
        || error_lower.contains("unexpected token")
    {
        return "syntax_error".into();
    }

    "unclassified".into()
}
