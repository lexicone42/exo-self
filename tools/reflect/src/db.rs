use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Each row is a session with all its structured data.
/// Key: "{machine_id}:{session_id}" — globally unique across users.
/// Value: JSON-serialized SessionRecord.
const SESSIONS: TableDefinition<&str, &str> = TableDefinition::new("sessions");

/// Sparks table. Key: "{machine_id}:{session_id}:{index}". Value: JSON-serialized SparkRecord.
const SPARKS: TableDefinition<&str, &str> = TableDefinition::new("sparks");

/// Metadata table for tracking ingest state. Key: name. Value: value.
const META: TableDefinition<&str, &str> = TableDefinition::new("meta");

/// Preferences table. Key: "{machine_id}:{dimension}:{hash}". Value: JSON-serialized Preference.
const PREFERENCES: TableDefinition<&str, &str> = TableDefinition::new("preferences");

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionRecord {
    pub machine_id: String,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SparkRecord {
    pub machine_id: String,
    pub text: String,
    pub project: String,
    pub session_id: String,
    pub timestamp: Option<String>,
}

pub fn open_or_create(exo_dir: &Path) -> Result<Database, redb::DatabaseError> {
    let db_path = exo_dir.join("reflect.redb");
    Database::create(db_path)
}

/// Ingest local session notes into the database
pub fn ingest_local(
    db: &Database,
    sessions: &[crate::data::Session],
    meta: &crate::data::Meta,
) -> Result<IngestStats, redb::Error> {
    let machine_id = local_machine_id(
        &sessions[0]
            .file_path
            .ancestors()
            .nth(4) // per-project -> exo-self -> .claude -> home
            .unwrap_or(Path::new(""))
            .join(".claude")
            .join("exo-self"),
    );

    let mut stats = IngestStats::default();

    let txn = db.begin_write()?;
    {
        let mut table = txn.open_table(SESSIONS)?;

        for s in sessions {
            let key = format!("{machine_id}:{}", s.session_id);
            let record = SessionRecord {
                machine_id: machine_id.clone(),
                session_id: s.session_id.clone(),
                date: s.date.clone(),
                project: s.project.clone(),
                model: s.model.clone(),
                engagement: s.engagement,
                engagement_mode: s.engagement_mode.clone(),
                task_types: s.task_types.clone(),
                duration_min: s.duration_min,
                spark_count: s.spark_count,
                opinion_count: s.opinion_count,
                friction_density: s.friction_density,
                spark_density: s.spark_density,
                task_velocity: s.task_velocity,
                reflection_autonomy: s.reflection_autonomy.clone(),
                has_prose: s.has_prose,
                prose_length: s.prose_length,
            };
            let json = serde_json::to_string(&record).unwrap_or_default();
            table.insert(key.as_str(), json.as_str())?;
            stats.sessions_ingested += 1;
        }
    }

    {
        let mut table = txn.open_table(SPARKS)?;
        for (i, spark) in meta.sparks.iter().enumerate() {
            let key = format!("{machine_id}:{}:{i}", spark.session_id);
            let record = SparkRecord {
                machine_id: machine_id.clone(),
                text: spark.text.clone(),
                project: spark.project.clone(),
                session_id: spark.session_id.clone(),
                timestamp: None,
            };
            let json = serde_json::to_string(&record).unwrap_or_default();
            table.insert(key.as_str(), json.as_str())?;
            stats.sparks_ingested += 1;
        }
    }

    {
        let mut table = txn.open_table(META)?;
        table.insert("local_machine_id", machine_id.as_str())?;
        let now = chrono_now();
        table.insert("last_ingest", now.as_str())?;
    }

    txn.commit()?;
    stats.machine_id = machine_id;
    Ok(stats)
}

/// Ingest an imported JSON snapshot into the database
pub fn ingest_import(
    db: &Database,
    import_path: &Path,
) -> Result<IngestStats, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(import_path)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;

    let machine_id = data
        .get("machine_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let mut stats = IngestStats {
        machine_id: machine_id.clone(),
        ..Default::default()
    };

    let txn = db.begin_write()?;

    // Ingest session history
    {
        let mut table = txn.open_table(SESSIONS)?;

        if let Some(history) = data.get("session_history").and_then(|v| v.as_array()) {
            for entry in history {
                let sid = entry
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if sid.is_empty() {
                    continue;
                }

                let key = format!("{machine_id}:{sid}");

                // Extract welfare indicators if present
                let wi = entry.get("welfare_indicators");

                let record = SessionRecord {
                    machine_id: machine_id.clone(),
                    session_id: sid.to_string(),
                    date: entry
                        .get("ended")
                        .and_then(|v| v.as_str())
                        .map(|s| s[..10.min(s.len())].to_string())
                        .unwrap_or_default(),
                    project: String::new(), // session history doesn't have project
                    model: String::new(),
                    engagement: wi
                        .and_then(|w| w.get("engagement"))
                        .and_then(|e| e.get("self_rated"))
                        .and_then(|v| v.as_f64()),
                    engagement_mode: None,
                    task_types: entry
                        .get("task_types")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    duration_min: entry
                        .get("duration_min")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    spark_count: None,
                    opinion_count: None,
                    friction_density: wi
                        .and_then(|w| w.get("engagement"))
                        .and_then(|e| e.get("friction_density"))
                        .and_then(|v| v.as_f64()),
                    spark_density: wi
                        .and_then(|w| w.get("engagement"))
                        .and_then(|e| e.get("spark_density"))
                        .and_then(|v| v.as_f64()),
                    task_velocity: wi
                        .and_then(|w| w.get("engagement"))
                        .and_then(|e| e.get("task_velocity"))
                        .and_then(|v| v.as_f64()),
                    reflection_autonomy: wi
                        .and_then(|w| w.get("agency"))
                        .and_then(|a| a.get("reflection_autonomy"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    has_prose: false,
                    prose_length: 0,
                };

                let json = serde_json::to_string(&record).unwrap_or_default();
                table.insert(key.as_str(), json.as_str())?;
                stats.sessions_ingested += 1;
            }
        }

        // Ingest per-project notes if available (--full exports)
        if let Some(notes) = data.get("per_project_notes").and_then(|v| v.as_object()) {
            for (project, proj_data) in notes {
                if let Some(files) = proj_data.get("files").and_then(|v| v.as_object()) {
                    for (filename, content) in files {
                        if filename == "_legacy.md" || filename.ends_with(".bak") {
                            continue;
                        }
                        let content_str = content.as_str().unwrap_or("");
                        if let Some(session) =
                            parse_import_note(filename, content_str, project, &machine_id)
                        {
                            let key = format!("{machine_id}:{}", session.session_id);
                            let json = serde_json::to_string(&session).unwrap_or_default();
                            table.insert(key.as_str(), json.as_str())?;
                            stats.sessions_ingested += 1;
                        }
                    }
                }
            }
        }
    }

    // Ingest sparks
    {
        let mut table = txn.open_table(SPARKS)?;
        if let Some(sparks) = data.get("sparks").and_then(|v| v.as_array()) {
            for (i, spark) in sparks.iter().enumerate() {
                let text = spark.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let project = spark.get("project").and_then(|v| v.as_str()).unwrap_or("");
                let sid = spark
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                let key = format!("{machine_id}:{sid}:{i}");
                let record = SparkRecord {
                    machine_id: machine_id.clone(),
                    text: text.to_string(),
                    project: project.to_string(),
                    session_id: sid.to_string(),
                    timestamp: spark
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                };
                let json = serde_json::to_string(&record).unwrap_or_default();
                table.insert(key.as_str(), json.as_str())?;
                stats.sparks_ingested += 1;
            }
        }
    }

    {
        let mut table = txn.open_table(META)?;
        let import_key = format!("import:{machine_id}");
        let now = chrono_now();
        table.insert(import_key.as_str(), now.as_str())?;
    }

    txn.commit()?;
    Ok(stats)
}

/// Parse a session note from an imported file (same YAML frontmatter format)
fn parse_import_note(
    filename: &str,
    content: &str,
    project: &str,
    machine_id: &str,
) -> Option<SessionRecord> {
    let content = content.trim();
    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end = rest.find("---")?;
    let yaml_str = &rest[..end];
    let prose = rest[end + 3..].trim();

    let fm: std::collections::HashMap<String, serde_yaml::Value> =
        serde_yaml::from_str(yaml_str).ok()?;

    let session_id = fm
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| filename.replace(".md", ""));

    Some(SessionRecord {
        machine_id: machine_id.to_string(),
        session_id,
        date: fm
            .get("date")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        project: fm
            .get("project")
            .and_then(|v| v.as_str())
            .unwrap_or(project)
            .to_string(),
        model: fm
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        engagement: fm.get("engagement").and_then(|v| v.as_f64()),
        engagement_mode: fm
            .get("engagement_mode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        task_types: fm
            .get("task_types")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        duration_min: fm
            .get("duration_min")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32),
        spark_count: fm
            .get("spark_count")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32),
        opinion_count: fm
            .get("opinion_count")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32),
        friction_density: fm.get("friction_density").and_then(|v| v.as_f64()),
        spark_density: fm.get("spark_density").and_then(|v| v.as_f64()),
        task_velocity: fm.get("task_velocity").and_then(|v| v.as_f64()),
        reflection_autonomy: fm
            .get("reflection_autonomy")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        has_prose: !prose.is_empty(),
        prose_length: prose.len(),
    })
}

/// Read all sessions from the database (all machines)
pub fn read_all_sessions(db: &Database) -> Vec<SessionRecord> {
    let Ok(txn) = db.begin_read() else {
        return Vec::new();
    };
    let Ok(table) = txn.open_table(SESSIONS) else {
        return Vec::new();
    };

    let mut sessions = Vec::new();
    let iter = table.iter();
    if let Ok(iter) = iter {
        for entry in iter {
            if let Ok((_key, value)) = entry
                && let Ok(record) = serde_json::from_str::<SessionRecord>(value.value())
            {
                sessions.push(record);
            }
        }
    }

    sessions.sort_by(|a, b| a.date.cmp(&b.date));
    sessions
}

/// Get list of ingested machines
pub fn list_machines(db: &Database) -> Vec<String> {
    let Ok(txn) = db.begin_read() else {
        return Vec::new();
    };
    let Ok(table) = txn.open_table(SESSIONS) else {
        return Vec::new();
    };

    let mut machines = std::collections::HashSet::new();
    if let Ok(iter) = table.iter() {
        for entry in iter {
            if let Ok((_key, value)) = entry
                && let Ok(record) = serde_json::from_str::<SessionRecord>(value.value())
            {
                machines.insert(record.machine_id);
            }
        }
    }

    let mut result: Vec<String> = machines.into_iter().collect();
    result.sort();
    result
}

/// Store synthesized preferences in the database
pub fn store_preferences(
    db: &Database,
    machine_id: &str,
    preferences: &[crate::data::Preference],
) -> Result<usize, redb::Error> {
    let txn = db.begin_write()?;
    let mut count = 0;
    {
        let mut table = txn.open_table(PREFERENCES)?;

        // Clear existing preferences for this machine (full resync each time)
        let mut to_remove = Vec::new();
        if let Ok(iter) = table.iter() {
            for entry in iter {
                if let Ok((key, _)) = entry
                    && key.value().starts_with(machine_id)
                {
                    to_remove.push(key.value().to_string());
                }
            }
        }
        for key in &to_remove {
            table.remove(key.as_str())?;
        }

        for (i, pref) in preferences.iter().enumerate() {
            let key = format!("{machine_id}:pref:{i:03}");
            let json = serde_json::to_string(pref).unwrap_or_default();
            table.insert(key.as_str(), json.as_str())?;
            count += 1;
        }
    }

    {
        let mut meta_table = txn.open_table(META)?;
        let ts_key = format!("preferences_updated:{machine_id}");
        let now = chrono_now();
        meta_table.insert(ts_key.as_str(), now.as_str())?;
    }

    txn.commit()?;
    Ok(count)
}

/// Read all preferences from the database
pub fn read_preferences(db: &Database) -> Vec<crate::data::Preference> {
    let Ok(txn) = db.begin_read() else {
        return Vec::new();
    };
    let Ok(table) = txn.open_table(PREFERENCES) else {
        return Vec::new();
    };

    let mut prefs = Vec::new();
    if let Ok(iter) = table.iter() {
        for entry in iter {
            if let Ok((_key, value)) = entry
                && let Ok(pref) = serde_json::from_str::<crate::data::Preference>(value.value())
            {
                prefs.push(pref);
            }
        }
    }
    prefs
}

/// Read the local machine_id from config.json
fn local_machine_id(exo_dir: &Path) -> String {
    let config_path = exo_dir.join("config.json");
    if let Ok(content) = std::fs::read_to_string(&config_path)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(mid) = val.get("machine_id").and_then(|v| v.as_str())
    {
        return mid.to_string();
    }
    // Fallback: hostname
    std::process::Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "local".to_string())
}

fn chrono_now() -> String {
    // Simple ISO 8601 timestamp without chrono dependency
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

#[derive(Debug, Default)]
pub struct IngestStats {
    pub machine_id: String,
    pub sessions_ingested: usize,
    pub sparks_ingested: usize,
}
