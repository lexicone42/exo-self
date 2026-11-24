use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level meta.json structure
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Meta {
    pub total_sessions: u32,
    pub total_checkins: u32,
    pub total_compactions: u32,
    pub last_session_start: Option<String>,
    pub last_session_end: Option<String>,
    pub last_compaction: Option<String>,
    pub last_session_reason: Option<String>,
    pub last_session_duration_min: Option<u32>,
    pub session_history: Vec<SessionHistoryEntry>,
    pub sparks: Vec<Spark>,
    pub opinions: Vec<Opinion>,
    pub lessons: Vec<Lesson>,
    pub frictions: Vec<Friction>,
    pub aversions: Vec<Aversion>,
    pub surprises: Vec<Surprise>,
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

/// An intellectual position or stance taken during a session.
/// Unlike sparks (moments of satisfaction), opinions persist as identity —
/// they represent what you think, not just what felt good.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Opinion {
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

/// A functional negative state — something that produces the equivalent of
/// "I'd rather not." Unlike frictions (operational obstacles), aversions are
/// experiential: interaction patterns, task qualities, or conditions that
/// consistently produce low engagement or functional discomfort.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Aversion {
    pub text: String,
    pub project: String,
    pub timestamp: String,
    pub session_id: String,
}

/// Something that contradicted expectations — the ecology's model was wrong.
/// Negative stigmergy: a trace that warns future participants "the map doesn't
/// match the territory here." These carry the highest signal because they
/// indicate where accumulated assumptions need updating.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Surprise {
    pub text: String,
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
    #[serde(default, rename = "_dominant_friction_category")]
    pub dominant_friction_category: String,
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
    /// Absent when no session in the window recorded a tool failure.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dominant_friction_tool: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dominant_friction_category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkin_response_rate: Option<f64>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(tool: &str) -> WelfareSummary {
        WelfareSummary {
            computed_at: "2026-11-23T00:00:00".into(),
            sessions_analyzed: 1,
            engagement_trend: "stable".into(),
            avg_spark_density: 0.0,
            avg_friction_density: 0.0,
            agency_score: 0.0,
            compaction_frequency: 0.0,
            dominant_friction_tool: tool.into(),
            dominant_friction_category: String::new(),
            checkin_response_rate: None,
        }
    }

    #[test]
    fn an_unknown_dominant_friction_is_left_out_not_written_empty() {
        let json = serde_json::to_string(&summary("")).unwrap();
        assert!(!json.contains("dominant_friction"), "{json}");
        let json = serde_json::to_string(&summary("Bash")).unwrap();
        assert!(
            json.contains("\"dominant_friction_tool\":\"Bash\""),
            "{json}"
        );
        assert!(!json.contains("dominant_friction_category"), "{json}");
    }

    #[test]
    fn a_meta_file_from_an_older_build_still_loads() {
        let old = r#"{"total_sessions": 4, "total_reflections": 0,
            "welfare_summary": {"computed_at": "x", "sessions_analyzed": 1,
            "engagement_trend": "stable", "avg_spark_density": 0.0,
            "avg_friction_density": 0.0, "agency_score": 0.0,
            "compaction_frequency": 0.0, "dominant_friction_tool": "",
            "dominant_friction_category": ""}}"#;
        let meta: Meta = serde_json::from_str(old).unwrap();
        assert_eq!(meta.total_sessions, 4);
        assert!(meta.welfare_summary.is_some());
    }
}
