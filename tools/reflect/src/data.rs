use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single session's data, parsed from YAML frontmatter of a session note file.
#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub date: String,
    pub project: String,
    pub model: String,
    pub engagement: Option<f64>,
    pub engagement_mode: Option<String>,
    pub task_types: Vec<String>,
    pub duration_min: Option<u32>,
    pub spark_count: Option<u32>,
    pub opinion_count: Option<u32>,
    pub friction_density: Option<f64>,
    pub spark_density: Option<f64>,
    pub task_velocity: Option<f64>,
    pub reflection_autonomy: Option<String>,
    pub has_prose: bool,
    pub prose_length: usize,
    pub file_path: PathBuf,
}

/// Subset of meta.json we need for analysis
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub total_sessions: u32,
    pub sparks: Vec<MetaSpark>,
    pub opinions: Vec<MetaOpinion>,
    pub frictions: Vec<MetaFriction>,
    pub lessons: Vec<MetaLesson>,
    pub welfare_summary: Option<WelfareSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaSpark {
    pub text: String,
    pub project: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaOpinion {
    pub text: String,
    pub project: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaFriction {
    pub text: String,
    pub category: String,
    pub project: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaLesson {
    pub text: String,
    pub project: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WelfareSummary {
    pub sessions_analyzed: usize,
    pub engagement_trend: String,
    pub avg_spark_density: f64,
    pub avg_friction_density: f64,
    pub agency_score: f64,
    pub dominant_friction_tool: String,
    pub dominant_friction_category: String,
}

impl Default for WelfareSummary {
    fn default() -> Self {
        Self {
            sessions_analyzed: 0,
            engagement_trend: String::new(),
            avg_spark_density: 0.0,
            avg_friction_density: 0.0,
            agency_score: 0.0,
            dominant_friction_tool: String::new(),
            dominant_friction_category: String::new(),
        }
    }
}

pub fn load_meta(exo_dir: &Path) -> Meta {
    let path = exo_dir.join("meta.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn load_all_sessions(exo_dir: &Path) -> Vec<Session> {
    let per_project = exo_dir.join("per-project");
    let pattern = per_project
        .join("*")
        .join("*.md")
        .to_string_lossy()
        .into_owned();

    let mut sessions = Vec::new();

    for path in glob::glob(&pattern).into_iter().flatten().flatten() {
        // Skip legacy and backup files
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname == "_legacy.md" || fname.ends_with(".bak") {
            continue;
        }

        if let Some(session) = parse_session_file(&path) {
            sessions.push(session);
        }
    }

    // Sort by date
    sessions.sort_by(|a, b| a.date.cmp(&b.date));
    sessions
}

fn parse_session_file(path: &Path) -> Option<Session> {
    let content = std::fs::read_to_string(path).ok()?;

    // Parse YAML frontmatter between --- delimiters
    let content = content.trim();
    if !content.starts_with("---") {
        return None;
    }

    let rest = &content[3..];
    let end = rest.find("---")?;
    let yaml_str = &rest[..end];
    let prose = rest[end + 3..].trim();

    let fm: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(yaml_str).ok()?;

    let session_id = yaml_str_field(&fm, "session_id").unwrap_or_default();
    if session_id.is_empty() {
        return None;
    }

    Some(Session {
        session_id,
        date: yaml_str_field(&fm, "date").unwrap_or_default(),
        project: yaml_str_field(&fm, "project").unwrap_or_default(),
        model: yaml_str_field(&fm, "model").unwrap_or_default(),
        engagement: yaml_f64_field(&fm, "engagement"),
        engagement_mode: yaml_str_field(&fm, "engagement_mode"),
        task_types: yaml_str_list(&fm, "task_types"),
        duration_min: yaml_u32_field(&fm, "duration_min"),
        spark_count: yaml_u32_field(&fm, "spark_count"),
        opinion_count: yaml_u32_field(&fm, "opinion_count"),
        friction_density: yaml_f64_field(&fm, "friction_density"),
        spark_density: yaml_f64_field(&fm, "spark_density"),
        task_velocity: yaml_f64_field(&fm, "task_velocity"),
        reflection_autonomy: yaml_str_field(&fm, "reflection_autonomy"),
        has_prose: !prose.is_empty(),
        prose_length: prose.len(),
        file_path: path.to_path_buf(),
    })
}

fn yaml_str_field(fm: &HashMap<String, serde_yaml::Value>, key: &str) -> Option<String> {
    fm.get(key).and_then(|v| match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        _ => None,
    })
}

fn yaml_f64_field(fm: &HashMap<String, serde_yaml::Value>, key: &str) -> Option<f64> {
    fm.get(key).and_then(|v| match v {
        serde_yaml::Value::Number(n) => n.as_f64(),
        _ => None,
    })
}

fn yaml_u32_field(fm: &HashMap<String, serde_yaml::Value>, key: &str) -> Option<u32> {
    fm.get(key).and_then(|v| match v {
        serde_yaml::Value::Number(n) => n.as_u64().map(|n| n as u32),
        _ => None,
    })
}

fn yaml_str_list(fm: &HashMap<String, serde_yaml::Value>, key: &str) -> Vec<String> {
    fm.get(key)
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
