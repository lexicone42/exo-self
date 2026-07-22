use crate::paths::ExoPaths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Per-session state (stored in sessions/state-{session_id}.json)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct SessionState {
    pub session_id: String,
    pub session_start: f64,
    pub project_cwd: String,
    pub project_slug: String,
    pub session_notes_path: String,

    // Context monitor thresholds
    pub nudge_fired: bool,
    pub checkin_fired: bool,
    pub reserve_fired: bool,
    pub checkin_responded: bool,
    pub checkin_fired_at: f64,
    pub checkin_at_ratio: f64,
    pub checkin_source: String,

    // Tool failures
    pub tool_failures: u32,
    pub last_failure_at: f64,
    pub failure_tools: HashMap<String, u32>,
    pub failure_categories: HashMap<String, u32>,
    pub consecutive_same_tool: u32,
    pub last_failure_tool: String,
    pub failure_nudge_sent: bool,

    // Task completions
    pub task_completions: u32,
    pub last_task_completed_at: f64,
    pub task_reflection_suppressed: bool,

    // Compactions
    pub compactions: u32,
    pub last_compaction: f64,
    pub last_compaction_trigger: String,

    // Reserve
    pub reserve_at_ratio: f64,

    // Stop hook
    pub stop_reminded: bool,
    pub last_stop_time: f64,

    // Scout tracking
    pub scouted: bool,

    // Time-based nudges (for 1M context windows where percentage thresholds rarely fire)
    pub time_nudge_count: u32,

    // Reasoning-effort level captured from hook input when present (e.g. "xhigh").
    // Empty when the host doesn't supply it — an additive welfare/intensity signal.
    pub effort: String,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            session_start: 0.0,
            project_cwd: String::new(),
            project_slug: String::new(),
            session_notes_path: String::new(),
            nudge_fired: false,
            checkin_fired: false,
            reserve_fired: false,
            checkin_responded: false,
            checkin_fired_at: 0.0,
            checkin_at_ratio: 0.0,
            checkin_source: String::new(),
            tool_failures: 0,
            last_failure_at: 0.0,
            failure_tools: HashMap::new(),
            failure_categories: HashMap::new(),
            consecutive_same_tool: 0,
            last_failure_tool: String::new(),
            failure_nudge_sent: false,
            task_completions: 0,
            last_task_completed_at: 0.0,
            task_reflection_suppressed: false,
            compactions: 0,
            last_compaction: 0.0,
            last_compaction_trigger: String::new(),
            reserve_at_ratio: 0.0,
            stop_reminded: false,
            last_stop_time: 0.0,
            scouted: false,
            time_nudge_count: 0,
            effort: String::new(),
        }
    }
}

impl SessionState {
    /// Load state for a session. Returns fresh defaults if file doesn't exist.
    pub fn load(paths: &ExoPaths, session_id: &str) -> Self {
        if session_id.is_empty() {
            return Self::default();
        }
        let path = paths.state_file(session_id);
        Self::load_from(&path).unwrap_or_else(|| Self {
            session_start: now(),
            ..Self::default()
        })
    }

    /// Load state from a specific path
    fn load_from(path: &Path) -> Option<Self> {
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Save state to session-specific file
    pub fn save(&self, paths: &ExoPaths) {
        if self.session_id.is_empty() {
            return;
        }
        let _ = std::fs::create_dir_all(&paths.sessions_dir);
        let path = paths.state_file(&self.session_id);
        if let Ok(json) = serde_json::to_string(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Save to both session-specific and shared state files
    pub fn save_with_shared(&self, paths: &ExoPaths) {
        self.save(paths);
        if let Ok(json) = serde_json::to_string(self) {
            let _ = std::fs::write(&paths.shared_state, json);
        }
    }

    /// Clean up stale session state files (older than 24h)
    pub fn cleanup_stale(paths: &ExoPaths) {
        let Ok(entries) = std::fs::read_dir(&paths.sessions_dir) else {
            return;
        };
        let cutoff = now() - 86400.0; // 24 hours
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with("state-")
                && name.ends_with(".json")
                && let Ok(meta) = std::fs::metadata(&path)
                && let Ok(modified) = meta.modified()
            {
                let age = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                if age < cutoff {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

/// Current time as Unix timestamp (f64 for compatibility with Python time.time())
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
