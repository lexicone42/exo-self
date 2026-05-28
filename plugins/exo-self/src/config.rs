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
            estimated_max_chars: 4_000_000,
            nudge_threshold: 0.60,
            checkin_threshold: 0.75,
            reserve_threshold: 0.88,
            max_journal_chars: 1500,
            max_journal_entries: 2,
            max_interests_items: 5,
            max_sparks_display: 5,
            failure_nudge_threshold: 10,
            merge_plugins: Vec::new(),
        }
    }
}

/// Legacy `estimated_max_chars` default shipped by older setup.sh: 800K chars ≈ a
/// 200K-token window. On modern 1M-token models this saturates the filesize-based
/// usage ratio and fires context nudges from the start of a session. It also disables
/// content-scaling in `scaling.rs` (which treats 800K as the no-scale baseline).
const LEGACY_MAX_CHARS: u64 = 800_000;

impl Config {
    pub fn load(path: &Path) -> Self {
        let mut cfg: Self = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        cfg.migrate();
        cfg
    }

    /// One-time, in-memory migration of legacy persisted defaults.
    ///
    /// Older setup.sh wrote `estimated_max_chars: 800000` into users' config.json, and
    /// the `[ ! -f ]` guard in setup.sh means that value is never corrected on update —
    /// a once-written default becomes permanent. We treat the exact legacy sentinel as
    /// "unset" and fall through to the current default. Applied in-memory only; the
    /// user's file is never rewritten. Removable once old configs have aged out.
    fn migrate(&mut self) {
        if self.estimated_max_chars == LEGACY_MAX_CHARS {
            self.estimated_max_chars = Self::default().estimated_max_chars;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(json: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(json.as_bytes()).unwrap();
        f
    }

    #[test]
    fn legacy_max_chars_is_migrated() {
        let f = write_config(r#"{"estimated_max_chars": 800000}"#);
        let cfg = Config::load(f.path());
        assert_eq!(cfg.estimated_max_chars, 4_000_000);
    }

    #[test]
    fn modern_max_chars_is_preserved() {
        let f = write_config(r#"{"estimated_max_chars": 4000000}"#);
        let cfg = Config::load(f.path());
        assert_eq!(cfg.estimated_max_chars, 4_000_000);
    }

    #[test]
    fn deliberate_non_legacy_value_is_preserved() {
        // A user who sets something other than the legacy sentinel keeps it.
        let f = write_config(r#"{"estimated_max_chars": 1500000}"#);
        let cfg = Config::load(f.path());
        assert_eq!(cfg.estimated_max_chars, 1_500_000);
    }

    #[test]
    fn missing_config_uses_default() {
        let cfg = Config::load(Path::new("/nonexistent/config.json"));
        assert_eq!(cfg.estimated_max_chars, 4_000_000);
    }

    #[test]
    fn other_fields_unaffected_by_migration() {
        let f = write_config(r#"{"estimated_max_chars": 800000, "nudge_threshold": 0.5}"#);
        let cfg = Config::load(f.path());
        assert_eq!(cfg.estimated_max_chars, 4_000_000);
        assert_eq!(cfg.nudge_threshold, 0.5);
    }
}
