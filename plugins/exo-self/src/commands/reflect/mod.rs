//! reflect — reflexive analysis of exo-self session data.
//!
//! Engagement correlations and preference inference with provenance
//! tracking, computed from the session notes and `meta.json` on this machine.
//! Nothing is stored: every run reads the files again.

mod analysis;
pub mod data;
mod report;

use std::path::Path;

pub fn run() {
    run_file_report(&exo_dir());
}

fn run_file_report(exo_dir: &Path) {
    let sessions = data::load_all_sessions(exo_dir);
    if sessions.is_empty() {
        eprintln!("No session data found in {}", exo_dir.display());
        std::process::exit(1);
    }

    let meta = data::load_meta(exo_dir);
    run_analysis(&sessions, &meta);
}

fn run_analysis(sessions: &[data::Session], meta: &data::Meta) {
    let engagement = analysis::engagement_correlations(sessions);
    let projects = analysis::project_breakdown(sessions);
    let task_types = analysis::task_type_analysis(sessions);
    let temporal = analysis::temporal_trends(sessions);
    let predictions = analysis::engagement_predictors(sessions);
    let spark_analysis = analysis::spark_patterns(sessions, meta);
    let preferences = analysis::infer_preferences(sessions, meta);

    report::print_report(
        sessions,
        meta,
        &engagement,
        &projects,
        &task_types,
        &temporal,
        &predictions,
        &spark_analysis,
        &preferences,
    );
}

fn exo_dir() -> std::path::PathBuf {
    crate::paths::root()
}
