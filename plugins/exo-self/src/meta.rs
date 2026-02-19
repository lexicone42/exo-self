use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level meta.json structure
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Meta {
    pub total_sessions: u32,
    pub total_checkins: u32,
    pub total_reflections: u32,
    pub total_compactions: u32,
    pub last_session_start: Option<String>,
    pub last_session_end: Option<String>,
    pub last_compaction: Option<String>,
    pub last_session_reason: Option<String>,
    pub last_session_duration_min: Option<u32>,
    pub session_history: Vec<SessionHistoryEntry>,
    pub sparks: Vec<Spark>,
    pub lessons: Vec<Lesson>,
    pub frictions: Vec<Friction>,
    pub welfare_summary: Option<WelfareSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionHistoryEntry {
    pub session_id: String,
    pub ended: String,
    pub reason: String,
    pub duration_min: u32,
    pub checkin_fired: bool,
    pub checkin_responded: bool,
    pub compactions: u32,
    #[serde(default)]
    pub plan_mode_used: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub welfare_indicators: Option<WelfareIndicators>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Spark {
    pub text: String,
    pub project: String,
    pub timestamp: String,
    pub session_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lesson {
    pub text: String,
    pub project: String,
    pub timestamp: String,
    pub session_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Friction {
    pub text: String,
    pub category: String,
    pub project: String,
    pub timestamp: String,
    pub session_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WelfareIndicators {
    pub engagement: EngagementIndicators,
    pub agency: AgencyIndicators,
    pub continuity: ContinuityIndicators,
    pub metacognition: MetacognitionIndicators,
    #[serde(rename = "_dominant_failure_tool")]
    pub dominant_failure_tool: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EngagementIndicators {
    pub spark_density: f64,
    pub task_velocity: f64,
    pub friction_density: f64,
    pub checkin_responded: bool,
    pub self_rated: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgencyIndicators {
    pub reflection_autonomy: String,
    pub interest_explored: bool,
    pub autonomous_sparks: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContinuityIndicators {
    pub compaction_count: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetacognitionIndicators {
    pub error_trajectory: String,
    pub strategy_adaptation: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WelfareSummary {
    pub computed_at: String,
    pub sessions_analyzed: usize,
    pub engagement_trend: String,
    pub avg_spark_density: f64,
    pub avg_friction_density: f64,
    pub agency_score: f64,
    pub compaction_frequency: f64,
    pub dominant_friction_tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkin_response_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_mode_rate: Option<f64>,
}

impl Meta {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}
