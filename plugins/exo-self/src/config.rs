use serde::Deserialize;
use std::path::Path;

/// All config.json fields with sane defaults
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub estimated_max_chars: u64,
    pub nudge_threshold: f64,
    pub checkin_threshold: f64,
    pub reserve_threshold: f64,
    pub max_journal_chars: usize,
    pub max_journal_entries: usize,
    pub max_interests_items: usize,
    pub max_sparks_display: usize,
    pub failure_nudge_threshold: u32,
    pub merge_plugins: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            estimated_max_chars: 800_000,
            nudge_threshold: 0.50,
            checkin_threshold: 0.65,
            reserve_threshold: 0.78,
            max_journal_chars: 1500,
            max_journal_entries: 2,
            max_interests_items: 5,
            max_sparks_display: 5,
            failure_nudge_threshold: 10,
            merge_plugins: Vec::new(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}
