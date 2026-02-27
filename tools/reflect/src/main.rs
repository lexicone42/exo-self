mod analysis;
mod data;
mod db;
mod report;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exo_dir = dirs::exo_dir();

    if args.iter().any(|a| a == "--ingest") {
        run_ingest(&exo_dir);
    } else if args.iter().any(|a| a == "--db") {
        run_db_report(&exo_dir);
    } else {
        // Default: read from files (no redb dependency required for basic use)
        run_file_report(&exo_dir);
    }
}

/// Ingest local session notes + imports into redb
fn run_ingest(exo_dir: &std::path::Path) {
    let database = db::open_or_create(exo_dir).expect("Failed to open/create reflect.redb");

    // Ingest local session notes
    let sessions = data::load_all_sessions(exo_dir);
    let meta = data::load_meta(exo_dir);

    if !sessions.is_empty() {
        match db::ingest_local(&database, &sessions, &meta) {
            Ok(stats) => {
                eprintln!(
                    "Ingested {} local sessions, {} sparks (machine: {})",
                    stats.sessions_ingested, stats.sparks_ingested, stats.machine_id
                );
            }
            Err(e) => eprintln!("Error ingesting local data: {e}"),
        }
    }

    // Ingest imports
    let imports_dir = exo_dir.join("imports");
    if imports_dir.is_dir() {
        let pattern = imports_dir.join("*.json").to_string_lossy().into_owned();
        for path in glob::glob(&pattern).into_iter().flatten().flatten() {
            match db::ingest_import(&database, &path) {
                Ok(stats) => {
                    eprintln!(
                        "Ingested import '{}': {} sessions, {} sparks (machine: {})",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        stats.sessions_ingested,
                        stats.sparks_ingested,
                        stats.machine_id,
                    );
                }
                Err(e) => eprintln!("Error ingesting {}: {e}", path.display()),
            }
        }
    }

    // Synthesize and store preferences
    if !sessions.is_empty() {
        let preferences = analysis::infer_preferences(&sessions, &meta);
        let machine_id = db::list_machines(&database)
            .into_iter()
            .next()
            .unwrap_or_else(|| "local".to_string());
        match db::store_preferences(&database, &machine_id, &preferences) {
            Ok(n) => eprintln!("Stored {n} inferred preferences"),
            Err(e) => eprintln!("Error storing preferences: {e}"),
        }
    }

    eprintln!("\nDatabase: {}", exo_dir.join("reflect.redb").display());
    let machines = db::list_machines(&database);
    eprintln!("Machines: {}", machines.join(", "));
}

/// Run report from redb database (cross-machine capable)
fn run_db_report(exo_dir: &std::path::Path) {
    let database = db::open_or_create(exo_dir).expect("Failed to open reflect.redb");
    let db_sessions = db::read_all_sessions(&database);

    if db_sessions.is_empty() {
        eprintln!("No data in reflect.redb. Run with --ingest first.");
        std::process::exit(1);
    }

    // Convert db records to analysis-compatible Sessions
    let sessions: Vec<data::Session> = db_sessions
        .iter()
        .map(|r| data::Session {
            session_id: r.session_id.clone(),
            date: r.date.clone(),
            project: if r.project.is_empty() {
                format!("{}:unknown", r.machine_id)
            } else {
                format!("{}:{}", r.machine_id, r.project)
            },
            model: r.model.clone(),
            engagement: r.engagement,
            engagement_mode: r.engagement_mode.clone(),
            task_types: r.task_types.clone(),
            duration_min: r.duration_min,
            spark_count: r.spark_count,
            opinion_count: r.opinion_count,
            friction_density: r.friction_density,
            spark_density: r.spark_density,
            task_velocity: r.task_velocity,
            reflection_autonomy: r.reflection_autonomy.clone(),
            has_prose: r.has_prose,
            prose_length: r.prose_length,
            file_path: std::path::PathBuf::new(),
        })
        .collect();

    let meta = data::load_meta(exo_dir);

    let machines = db::list_machines(&database);
    eprintln!(
        "Reporting from redb ({} sessions across {} machines: {})",
        sessions.len(),
        machines.len(),
        machines.join(", ")
    );

    run_analysis(&sessions, &meta);
}

/// Run report from local files only (default, no redb needed)
fn run_file_report(exo_dir: &std::path::Path) {
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

mod dirs {
    use std::path::PathBuf;

    pub fn exo_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());
        PathBuf::from(home).join(".claude").join("exo-self")
    }
}
