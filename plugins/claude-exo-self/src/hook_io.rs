use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

/// Common fields from Claude Code hook input (stdin JSON)
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct HookInput {
    pub session_id: String,
    pub cwd: String,
    pub transcript_path: String,
    pub trigger: String,
    pub reason: String,
    pub tool_name: String,
    pub stop_hook_active: bool,
}

impl HookInput {
    /// Read and parse hook input from stdin. Returns defaults on any error.
    pub fn from_stdin() -> Self {
        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_err() {
            return Self::default();
        }
        serde_json::from_str(&buf).unwrap_or_default()
    }
}

/// Hook output with additionalContext injection
#[derive(Serialize)]
pub struct HookOutput {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Serialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "additionalContext")]
    pub additional_context: String,
}

/// Stop hook decision output
#[derive(Serialize)]
pub struct DecisionOutput {
    pub decision: String,
    pub reason: String,
}

/// Print a hook output with additionalContext
pub fn hook_output(event_name: &str, context: &str) {
    let out = HookOutput {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: event_name.to_string(),
            additional_context: context.to_string(),
        },
    };
    println!("{}", serde_json::to_string(&out).unwrap_or_else(|_| "{}".into()));
}

/// Print a stop-hook decision
pub fn decision_output(decision: &str, reason: &str) {
    let out = DecisionOutput {
        decision: decision.to_string(),
        reason: reason.to_string(),
    };
    println!("{}", serde_json::to_string(&out).unwrap_or_else(|_| "{}".into()));
}

/// Print empty JSON (no-op output)
pub fn empty_output() {
    println!("{{}}");
}

/// Read raw stdin as a serde_json::Value (for statusline which needs full JSON)
pub fn raw_stdin() -> Value {
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return Value::Object(Default::default());
    }
    serde_json::from_str(&buf).unwrap_or_else(|_| Value::Object(Default::default()))
}
