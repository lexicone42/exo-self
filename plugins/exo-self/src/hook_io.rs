use serde::Serialize;
use serde_json::Value;
use std::io::Read;

/// Common fields from Claude Code hook input (stdin JSON)
#[derive(Debug, Default)]
pub struct HookInput {
    pub session_id: String,
    pub cwd: String,
    pub transcript_path: String,
    pub trigger: String,
    /// SessionStart only: "startup", "resume", "clear" or "compact".
    pub source: String,
    pub reason: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub error: String,
    pub stop_hook_active: bool,
    pub agent_type: String,
    /// Reasoning-effort level, when the host supplies it (empty otherwise). Spike (#19
    /// follow-up): confirmed present in this Claude Code's transcript records; captured
    /// gracefully to verify empirically which hook events actually carry it in stdin.
    pub effort: String,
}

impl HookInput {
    /// Read and parse hook input from stdin. Returns defaults on any error.
    pub fn from_stdin() -> Self {
        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_err() {
            return Self::default();
        }
        Self::parse(&buf)
    }

    /// Parse hook input, one field at a time.
    ///
    /// Parsing the whole struct at once meant one field of an unexpected type
    /// (the host sending `error` as an object, say) discarded every field,
    /// the session id included. Handlers then saw an empty session id and
    /// returned a no-op `{}`, which is also what a healthy run prints, so the
    /// failure and task counters sat at zero with nothing to show for it. Each
    /// field now stands alone: a string is taken as is, any other value is kept
    /// as its JSON text, and a field that is absent stays at its default.
    pub fn parse(raw: &str) -> Self {
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(raw) else {
            return Self::default();
        };
        let text = |key: &str| match map.get(key) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => String::new(),
            Some(other) => other.to_string(),
        };
        let mut input = Self {
            // An id is used in file names, so only a string is taken as one. A
            // value of any other type is left empty and the transcript name
            // below supplies the id.
            session_id: match map.get("session_id") {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            },
            cwd: text("cwd"),
            transcript_path: text("transcript_path"),
            trigger: text("trigger"),
            source: text("source"),
            reason: text("reason"),
            tool_name: text("tool_name"),
            tool_input: map.get("tool_input").cloned().unwrap_or(Value::Null),
            error: text("error"),
            stop_hook_active: match map.get("stop_hook_active") {
                Some(Value::Bool(b)) => *b,
                Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
                _ => false,
            },
            agent_type: text("agent_type"),
            effort: text("effort"),
        };
        // A transcript is named for its session. Use that when the id is missing.
        if input.session_id.is_empty()
            && let Some(stem) = std::path::Path::new(&input.transcript_path)
                .file_stem()
                .and_then(|s| s.to_str())
            && input.transcript_path.ends_with(".jsonl")
        {
            input.session_id = stem.to_string();
        }
        input
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
    println!(
        "{}",
        serde_json::to_string(&out).unwrap_or_else(|_| "{}".into())
    );
}

/// Print a stop-hook decision
pub fn decision_output(decision: &str, reason: &str) {
    let out = DecisionOutput {
        decision: decision.to_string(),
        reason: reason.to_string(),
    };
    println!(
        "{}",
        serde_json::to_string(&out).unwrap_or_else(|_| "{}".into())
    );
}

/// Print a PreToolUse permission decision (allow/deny)
pub fn pre_tool_use_decision(decision: &str, reason: &str) {
    let out = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
            "permissionDecisionReason": reason
        }
    });
    println!(
        "{}",
        serde_json::to_string(&out).unwrap_or_else(|_| "{}".into())
    );
}

/// Print empty JSON (no-op output)
pub fn empty_output() {
    println!("{{}}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_field_of_the_wrong_type_does_not_discard_the_session_id() {
        // The regression: any of these made the old whole-struct parse fall back
        // to defaults, so the handler saw no session and did nothing.
        for raw in [
            r#"{"session_id":"s1","tool_name":"Bash","error":{"message":"exit 3","code":3}}"#,
            r#"{"session_id":"s1","tool_name":"Bash","effort":{"level":"high"}}"#,
            r#"{"session_id":"s1","tool_name":"Bash","stop_hook_active":"false"}"#,
            r#"{"session_id":"s1","tool_name":"Bash","cwd":null,"permission_mode":7}"#,
        ] {
            let hi = HookInput::parse(raw);
            assert_eq!(hi.session_id, "s1", "{raw}");
            assert_eq!(hi.tool_name, "Bash", "{raw}");
        }
        let hi = HookInput::parse(r#"{"session_id":"s1","error":{"message":"exit 3"}}"#);
        assert_eq!(hi.error, r#"{"message":"exit 3"}"#);
    }

    #[test]
    fn well_formed_input_parses_as_before() {
        let hi = HookInput::parse(
            r#"{"session_id":"s","cwd":"/w","transcript_path":"/t/s.jsonl","trigger":"auto",
                "source":"resume","reason":"exit","tool_name":"Bash",
                "tool_input":{"command":"ls"},"error":"boom","stop_hook_active":true,
                "agent_type":"Explore","permission_mode":"auto","effort":"high",
                "an_unknown_field":[1,2,3]}"#,
        );
        assert_eq!(hi.session_id, "s");
        assert_eq!(hi.cwd, "/w");
        assert_eq!(hi.trigger, "auto");
        assert_eq!(hi.source, "resume");
        assert_eq!(hi.reason, "exit");
        assert_eq!(hi.tool_input["command"], "ls");
        assert_eq!(hi.error, "boom");
        assert!(hi.stop_hook_active);
        assert_eq!(hi.agent_type, "Explore");
        assert_eq!(hi.effort, "high");
    }

    #[test]
    fn a_missing_session_id_is_taken_from_the_transcript_name() {
        let hi = HookInput::parse(
            r#"{"transcript_path":"/p/ccff8de5-370f-454e-9c45-2d4c1e0b0453.jsonl","tool_name":"Bash"}"#,
        );
        assert_eq!(hi.session_id, "ccff8de5-370f-454e-9c45-2d4c1e0b0453");
        // A stated id wins, and a path that is not a transcript is not used.
        let hi = HookInput::parse(r#"{"session_id":"s","transcript_path":"/p/other.jsonl"}"#);
        assert_eq!(hi.session_id, "s");
        let hi = HookInput::parse(r#"{"transcript_path":"/p/notes.md"}"#);
        assert_eq!(hi.session_id, "");
    }

    #[test]
    fn input_that_is_not_a_json_object_gives_defaults() {
        for raw in ["", "not json", "[1,2]", "\"s\"", "null"] {
            assert_eq!(HookInput::parse(raw).session_id, "", "{raw}");
        }
    }

    #[test]
    fn hookinput_captures_effort_when_present() {
        let hi = HookInput::parse(r#"{"session_id":"s","effort":"xhigh"}"#);
        assert_eq!(hi.effort, "xhigh");
    }

    #[test]
    fn hookinput_effort_defaults_empty_when_absent() {
        // Graceful degradation: hosts/events without the field must not break parsing.
        let hi = HookInput::parse(r#"{"session_id":"s"}"#);
        assert_eq!(hi.effort, "");
    }

    #[test]
    fn a_session_id_of_the_wrong_type_falls_back_to_the_transcript_name() {
        for raw in [
            r#"{"session_id":{"id":"a/b"},"transcript_path":"/p/ccff8de5.jsonl"}"#,
            r#"{"session_id":123,"transcript_path":"/p/ccff8de5.jsonl"}"#,
            r#"{"session_id":[],"transcript_path":"/p/ccff8de5.jsonl"}"#,
            r#"{"session_id":null,"transcript_path":"/p/ccff8de5.jsonl"}"#,
        ] {
            assert_eq!(HookInput::parse(raw).session_id, "ccff8de5", "{raw}");
        }
        // With no transcript either, the id is empty, never JSON text.
        assert_eq!(
            HookInput::parse(r#"{"session_id":{"id":"x"}}"#).session_id,
            ""
        );
    }

    #[test]
    fn stop_hook_active_is_read_as_a_bool_or_the_word_true() {
        let flag = |raw: &str| HookInput::parse(raw).stop_hook_active;
        assert!(flag(r#"{"stop_hook_active":true}"#));
        assert!(flag(r#"{"stop_hook_active":"true"}"#));
        assert!(!flag(r#"{"stop_hook_active":false}"#));
        assert!(!flag(r#"{"stop_hook_active":"false"}"#));
        assert!(!flag(r#"{"stop_hook_active":"yes"}"#));
        assert!(!flag(r#"{}"#));
    }
}

/// Read raw stdin as a serde_json::Value (for statusline which needs full JSON)
pub fn raw_stdin() -> Value {
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return Value::Object(Default::default());
    }
    serde_json::from_str(&buf).unwrap_or_else(|_| Value::Object(Default::default()))
}
