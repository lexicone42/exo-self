use super::data::{
    Confidence, Meta, Preference, PreferenceDimension, Provenance, Session, Valence,
};
use std::collections::HashMap;

/// Pearson correlation between two f64 slices
fn pearson(xs: &[f64], ys: &[f64]) -> Option<f64> {
    let n = xs.len();
    if n < 3 || n != ys.len() {
        return None;
    }
    let n_f = n as f64;
    let mean_x = xs.iter().sum::<f64>() / n_f;
    let mean_y = ys.iter().sum::<f64>() / n_f;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for i in 0..n {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    let denom = (var_x * var_y).sqrt();
    if denom < 1e-10 {
        None
    } else {
        Some(cov / denom)
    }
}

/// Summary statistics for a set of values
#[derive(Debug, Clone)]
pub struct Stats {
    #[allow(dead_code)]
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub std_dev: f64,
}

impl Stats {
    pub fn from_values(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let n = values.len();
        let n_f = n as f64;
        let mean = values.iter().sum::<f64>() / n_f;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n_f;

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if n.is_multiple_of(2) {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        } else {
            sorted[n / 2]
        };

        Some(Stats {
            n,
            mean,
            median,
            min: sorted[0],
            max: sorted[n - 1],
            std_dev: variance.sqrt(),
        })
    }
}

/// Correlation between engagement and other continuous variables
#[derive(Debug)]
pub struct EngagementCorrelations {
    pub friction_vs_engagement: Option<f64>,
    pub spark_density_vs_engagement: Option<f64>,
    pub task_velocity_vs_engagement: Option<f64>,
    pub duration_vs_engagement: Option<f64>,
    pub prose_length_vs_engagement: Option<f64>,
    pub n_with_engagement: usize,
    pub engagement_stats: Option<Stats>,
}

pub fn engagement_correlations(sessions: &[Session]) -> EngagementCorrelations {
    let with_engagement: Vec<&Session> =
        sessions.iter().filter(|s| s.engagement.is_some()).collect();

    let engagements: Vec<f64> = with_engagement
        .iter()
        .map(|s| s.engagement.unwrap())
        .collect();

    let friction_vs = {
        let pairs: Vec<(f64, f64)> = with_engagement
            .iter()
            .filter_map(|s| s.friction_density.map(|f| (f, s.engagement.unwrap())))
            .collect();
        if pairs.len() >= 3 {
            let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
            pearson(&xs, &ys)
        } else {
            None
        }
    };

    let spark_vs = {
        let pairs: Vec<(f64, f64)> = with_engagement
            .iter()
            .filter_map(|s| s.spark_density.map(|sd| (sd, s.engagement.unwrap())))
            .collect();
        if pairs.len() >= 3 {
            let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
            pearson(&xs, &ys)
        } else {
            None
        }
    };

    let velocity_vs = {
        let pairs: Vec<(f64, f64)> = with_engagement
            .iter()
            .filter_map(|s| s.task_velocity.map(|tv| (tv, s.engagement.unwrap())))
            .collect();
        if pairs.len() >= 3 {
            let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
            pearson(&xs, &ys)
        } else {
            None
        }
    };

    let duration_vs = {
        let pairs: Vec<(f64, f64)> = with_engagement
            .iter()
            .filter_map(|s| s.duration_min.map(|d| (d as f64, s.engagement.unwrap())))
            .collect();
        if pairs.len() >= 3 {
            let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
            pearson(&xs, &ys)
        } else {
            None
        }
    };

    let prose_vs = {
        let pairs: Vec<(f64, f64)> = with_engagement
            .iter()
            .filter(|s| s.has_prose)
            .map(|s| (s.prose_length as f64, s.engagement.unwrap()))
            .collect();
        if pairs.len() >= 3 {
            let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
            pearson(&xs, &ys)
        } else {
            None
        }
    };

    EngagementCorrelations {
        friction_vs_engagement: friction_vs,
        spark_density_vs_engagement: spark_vs,
        task_velocity_vs_engagement: velocity_vs,
        duration_vs_engagement: duration_vs,
        prose_length_vs_engagement: prose_vs,
        n_with_engagement: with_engagement.len(),
        engagement_stats: Stats::from_values(&engagements),
    }
}

/// Per-project engagement and productivity summary
#[derive(Debug)]
pub struct ProjectSummary {
    pub project: String,
    pub session_count: usize,
    pub engagement: Option<Stats>,
    pub friction: Option<Stats>,
    pub spark_rate: f64,
    #[allow(dead_code)]
    pub top_task_types: Vec<(String, usize)>,
}

pub fn project_breakdown(sessions: &[Session]) -> Vec<ProjectSummary> {
    let mut by_project: HashMap<String, Vec<&Session>> = HashMap::new();
    for s in sessions {
        by_project.entry(s.project.clone()).or_default().push(s);
    }

    let mut summaries: Vec<ProjectSummary> = by_project
        .into_iter()
        .map(|(project, sessions)| {
            let engagements: Vec<f64> = sessions.iter().filter_map(|s| s.engagement).collect();
            let frictions: Vec<f64> = sessions.iter().filter_map(|s| s.friction_density).collect();
            let total_sparks: u32 = sessions.iter().filter_map(|s| s.spark_count).sum();

            let mut type_counts: HashMap<String, usize> = HashMap::new();
            for s in &sessions {
                for t in &s.task_types {
                    *type_counts.entry(t.clone()).or_insert(0) += 1;
                }
            }
            let mut top_types: Vec<(String, usize)> = type_counts.into_iter().collect();
            top_types.sort_by(|a, b| b.1.cmp(&a.1));
            top_types.truncate(5);

            ProjectSummary {
                session_count: sessions.len(),
                engagement: Stats::from_values(&engagements),
                friction: Stats::from_values(&frictions),
                spark_rate: if sessions.is_empty() {
                    0.0
                } else {
                    total_sparks as f64 / sessions.len() as f64
                },
                top_task_types: top_types,
                project,
            }
        })
        .collect();

    summaries.sort_by(|a, b| {
        let a_eng = a.engagement.as_ref().map(|s| s.mean).unwrap_or(0.0);
        let b_eng = b.engagement.as_ref().map(|s| s.mean).unwrap_or(0.0);
        b_eng
            .partial_cmp(&a_eng)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    summaries
}

/// Task type engagement analysis
#[derive(Debug)]
pub struct TaskTypeStats {
    pub task_type: String,
    pub count: usize,
    pub mean_engagement: f64,
    pub mean_friction: f64,
}

pub fn task_type_analysis(sessions: &[Session]) -> Vec<TaskTypeStats> {
    let mut by_type: HashMap<String, Vec<&Session>> = HashMap::new();
    for s in sessions {
        for t in &s.task_types {
            by_type.entry(t.clone()).or_default().push(s);
        }
    }

    let mut stats: Vec<TaskTypeStats> = by_type
        .into_iter()
        .filter(|(_, sessions)| sessions.len() >= 2)
        .map(|(task_type, sessions)| {
            let engagements: Vec<f64> = sessions.iter().filter_map(|s| s.engagement).collect();
            let frictions: Vec<f64> = sessions.iter().filter_map(|s| s.friction_density).collect();

            TaskTypeStats {
                count: sessions.len(),
                mean_engagement: if engagements.is_empty() {
                    0.0
                } else {
                    engagements.iter().sum::<f64>() / engagements.len() as f64
                },
                mean_friction: if frictions.is_empty() {
                    0.0
                } else {
                    frictions.iter().sum::<f64>() / frictions.len() as f64
                },
                task_type,
            }
        })
        .collect();

    stats.sort_by(|a, b| {
        b.mean_engagement
            .partial_cmp(&a.mean_engagement)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    stats
}

/// Temporal trend: engagement and friction over time windows
#[derive(Debug)]
pub struct TemporalTrend {
    pub window: String,
    pub session_count: usize,
    pub mean_engagement: Option<f64>,
    pub mean_friction: Option<f64>,
    pub spark_count: u32,
}

pub fn temporal_trends(sessions: &[Session]) -> Vec<TemporalTrend> {
    let mut by_week: HashMap<String, Vec<&Session>> = HashMap::new();
    for s in sessions {
        if s.date.len() >= 10 {
            let week = date_to_week(&s.date);
            by_week.entry(week).or_default().push(s);
        }
    }

    let mut trends: Vec<TemporalTrend> = by_week
        .into_iter()
        .map(|(window, sessions)| {
            let engagements: Vec<f64> = sessions.iter().filter_map(|s| s.engagement).collect();
            let frictions: Vec<f64> = sessions.iter().filter_map(|s| s.friction_density).collect();
            let sparks: u32 = sessions.iter().filter_map(|s| s.spark_count).sum();

            TemporalTrend {
                session_count: sessions.len(),
                mean_engagement: if engagements.is_empty() {
                    None
                } else {
                    Some(engagements.iter().sum::<f64>() / engagements.len() as f64)
                },
                mean_friction: if frictions.is_empty() {
                    None
                } else {
                    Some(frictions.iter().sum::<f64>() / frictions.len() as f64)
                },
                spark_count: sparks,
                window,
            }
        })
        .collect();

    trends.sort_by(|a, b| a.window.cmp(&b.window));
    trends
}

fn date_to_week(date: &str) -> String {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() >= 3
        && let (Ok(year), Ok(month), Ok(day)) = (
            parts[0].parse::<u32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        )
    {
        let day_of_year = rough_day_of_year(month, day);
        let week = (day_of_year - 1) / 7 + 1;
        return format!("{year}-W{week:02}");
    }
    "unknown".into()
}

fn rough_day_of_year(month: u32, day: u32) -> u32 {
    let days_before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let idx = (month as usize).saturating_sub(1).min(11);
    days_before[idx] + day
}

/// Engagement predictors: which features best predict high engagement?
#[derive(Debug)]
pub struct EngagementPredictors {
    pub high_engagement_projects: Vec<(String, f64)>,
    pub high_engagement_task_types: Vec<(String, f64)>,
    pub high_engagement_modes: Vec<(String, f64)>,
    pub autonomy_vs_engagement: Option<(f64, f64)>,
    pub prose_writing_vs_engagement: Option<(f64, f64)>,
}

pub fn engagement_predictors(sessions: &[Session]) -> EngagementPredictors {
    let mut proj_eng: HashMap<String, Vec<f64>> = HashMap::new();
    for s in sessions {
        if let Some(e) = s.engagement {
            proj_eng.entry(s.project.clone()).or_default().push(e);
        }
    }
    let mut high_projects: Vec<(String, f64)> = proj_eng
        .iter()
        .filter(|(_, v)| v.len() >= 3)
        .map(|(p, v)| (p.clone(), v.iter().sum::<f64>() / v.len() as f64))
        .collect();
    high_projects.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    high_projects.truncate(5);

    let mut type_eng: HashMap<String, Vec<f64>> = HashMap::new();
    for s in sessions {
        if let Some(e) = s.engagement {
            for t in &s.task_types {
                type_eng.entry(t.clone()).or_default().push(e);
            }
        }
    }
    let mut high_types: Vec<(String, f64)> = type_eng
        .iter()
        .filter(|(_, v)| v.len() >= 3)
        .map(|(t, v)| (t.clone(), v.iter().sum::<f64>() / v.len() as f64))
        .collect();
    high_types.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    high_types.truncate(7);

    let mut mode_eng: HashMap<String, Vec<f64>> = HashMap::new();
    for s in sessions {
        if let (Some(e), Some(mode)) = (s.engagement, &s.engagement_mode) {
            mode_eng.entry(mode.clone()).or_default().push(e);
        }
    }
    let mut high_modes: Vec<(String, f64)> = mode_eng
        .iter()
        .map(|(m, v)| (m.clone(), v.iter().sum::<f64>() / v.len() as f64))
        .collect();
    high_modes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let autonomous: Vec<f64> = sessions
        .iter()
        .filter(|s| s.reflection_autonomy.as_deref() == Some("autonomous"))
        .filter_map(|s| s.engagement)
        .collect();
    let prompted: Vec<f64> = sessions
        .iter()
        .filter(|s| s.reflection_autonomy.as_deref() == Some("prompted"))
        .filter_map(|s| s.engagement)
        .collect();
    let autonomy_split = if autonomous.len() >= 2 && prompted.len() >= 2 {
        Some((
            autonomous.iter().sum::<f64>() / autonomous.len() as f64,
            prompted.iter().sum::<f64>() / prompted.len() as f64,
        ))
    } else {
        None
    };

    let with_prose: Vec<f64> = sessions
        .iter()
        .filter(|s| s.has_prose)
        .filter_map(|s| s.engagement)
        .collect();
    let without_prose: Vec<f64> = sessions
        .iter()
        .filter(|s| !s.has_prose)
        .filter_map(|s| s.engagement)
        .collect();
    let prose_split = if with_prose.len() >= 2 && without_prose.len() >= 2 {
        Some((
            with_prose.iter().sum::<f64>() / with_prose.len() as f64,
            without_prose.iter().sum::<f64>() / without_prose.len() as f64,
        ))
    } else {
        None
    };

    EngagementPredictors {
        high_engagement_projects: high_projects,
        high_engagement_task_types: high_types,
        high_engagement_modes: high_modes,
        autonomy_vs_engagement: autonomy_split,
        prose_writing_vs_engagement: prose_split,
    }
}

/// Spark distribution analysis
#[derive(Debug)]
pub struct SparkPatterns {
    pub sparks_by_project: Vec<(String, usize)>,
    pub sessions_with_sparks: usize,
    pub sessions_without_sparks: usize,
    pub mean_engagement_with_sparks: Option<f64>,
    pub mean_engagement_without_sparks: Option<f64>,
}

pub fn spark_patterns(sessions: &[Session], _meta: &Meta) -> SparkPatterns {
    let with: Vec<&Session> = sessions
        .iter()
        .filter(|s| s.spark_count.unwrap_or(0) > 0)
        .collect();
    let without: Vec<&Session> = sessions
        .iter()
        .filter(|s| s.spark_count == Some(0))
        .collect();

    let eng_with: Vec<f64> = with.iter().filter_map(|s| s.engagement).collect();
    let eng_without: Vec<f64> = without.iter().filter_map(|s| s.engagement).collect();

    let mut by_project: HashMap<String, usize> = HashMap::new();
    for s in &with {
        *by_project.entry(s.project.clone()).or_insert(0) += s.spark_count.unwrap_or(0) as usize;
    }
    let mut spark_proj: Vec<(String, usize)> = by_project.into_iter().collect();
    spark_proj.sort_by(|a, b| b.1.cmp(&a.1));

    SparkPatterns {
        sparks_by_project: spark_proj,
        sessions_with_sparks: with.len(),
        sessions_without_sparks: without.len(),
        mean_engagement_with_sparks: if eng_with.is_empty() {
            None
        } else {
            Some(eng_with.iter().sum::<f64>() / eng_with.len() as f64)
        },
        mean_engagement_without_sparks: if eng_without.is_empty() {
            None
        } else {
            Some(eng_without.iter().sum::<f64>() / eng_without.len() as f64)
        },
    }
}

// ── Preference Inference ────────────────────────────────────────────

pub fn infer_preferences(sessions: &[Session], meta: &Meta) -> Vec<Preference> {
    let mut preferences = Vec::new();

    preferences.extend(infer_task_preferences(sessions));
    preferences.extend(infer_work_mode_preferences(sessions));
    preferences.extend(infer_autonomy_preferences(sessions));
    preferences.extend(infer_domain_preferences(sessions));
    preferences.extend(infer_boundary_preferences(meta, sessions));

    preferences.retain(|p| p.confidence.supporting > 0);

    preferences.sort_by(|a, b| {
        let a_strength = a.confidence.supporting as i64 - a.confidence.contradicting as i64;
        let b_strength = b.confidence.supporting as i64 - b.confidence.contradicting as i64;
        b_strength
            .cmp(&a_strength)
            .then(b.confidence.supporting.cmp(&a.confidence.supporting))
    });

    preferences
}

fn classify_provenance(first_supporting_idx: usize, total_sessions: usize) -> Provenance {
    if total_sessions < 5 {
        return Provenance::Developing;
    }
    if first_supporting_idx < 3 {
        Provenance::Trained
    } else if first_supporting_idx >= 10 || first_supporting_idx >= total_sessions / 3 {
        Provenance::Emergent
    } else {
        Provenance::Developing
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn infer_task_preferences(sessions: &[Session]) -> Vec<Preference> {
    let mut by_type: HashMap<String, Vec<(f64, &str, usize)>> = HashMap::new();
    for (idx, s) in sessions.iter().enumerate() {
        if let Some(e) = s.engagement {
            for t in &s.task_types {
                by_type
                    .entry(t.clone())
                    .or_default()
                    .push((e, &s.date, idx));
            }
        }
    }

    let mut prefs = Vec::new();

    for (task_type, observations) in &by_type {
        if observations.len() < 3 {
            continue;
        }

        let engagements: Vec<f64> = observations.iter().map(|(e, _, _)| *e).collect();
        let mean_eng = mean(&engagements);

        let (valence, claim) = if mean_eng >= 4.0 {
            (
                Valence::Approach,
                format!(
                    "{task_type} work → approach (mean engagement {mean_eng:.1}, n={})",
                    observations.len()
                ),
            )
        } else if mean_eng < 3.5 {
            (
                Valence::Avoid,
                format!(
                    "{task_type} work → avoid (mean engagement {mean_eng:.1}, n={})",
                    observations.len()
                ),
            )
        } else {
            continue;
        };

        let (supporting, contradicting) = match valence {
            Valence::Approach => (
                observations.iter().filter(|(e, _, _)| *e >= 4.0).count(),
                observations.iter().filter(|(e, _, _)| *e <= 2.5).count(),
            ),
            Valence::Avoid => (
                observations.iter().filter(|(e, _, _)| *e < 3.5).count(),
                observations.iter().filter(|(e, _, _)| *e >= 4.5).count(),
            ),
            Valence::Boundary => unreachable!(),
        };

        let first_supporting_idx = match valence {
            Valence::Approach => observations
                .iter()
                .filter(|(e, _, _)| *e >= 4.0)
                .map(|(_, _, idx)| *idx)
                .min(),
            Valence::Avoid => observations
                .iter()
                .filter(|(e, _, _)| *e < 3.5)
                .map(|(_, _, idx)| *idx)
                .min(),
            Valence::Boundary => None,
        };

        let first_date = observations
            .iter()
            .min_by_key(|(_, d, _)| *d)
            .map(|(_, d, _)| *d)
            .unwrap_or("");
        let last_date = observations
            .iter()
            .max_by_key(|(_, d, _)| *d)
            .map(|(_, d, _)| *d)
            .unwrap_or("");

        let provenance = classify_provenance(first_supporting_idx.unwrap_or(0), sessions.len());

        prefs.push(Preference {
            claim,
            dimension: PreferenceDimension::Task,
            valence,
            confidence: Confidence {
                supporting,
                contradicting,
            },
            first_observed: first_date.to_string(),
            last_confirmed: last_date.to_string(),
            provenance,
        });
    }

    prefs
}

fn infer_work_mode_preferences(sessions: &[Session]) -> Vec<Preference> {
    let mut by_mode: HashMap<String, Vec<(f64, &str, usize)>> = HashMap::new();
    for (idx, s) in sessions.iter().enumerate() {
        if let (Some(e), Some(mode)) = (s.engagement, &s.engagement_mode) {
            by_mode
                .entry(mode.clone())
                .or_default()
                .push((e, &s.date, idx));
        }
    }

    if by_mode.len() < 2 {
        return Vec::new();
    }

    let mut prefs = Vec::new();

    let all_engagements: Vec<f64> = by_mode
        .values()
        .flat_map(|v| v.iter().map(|(e, _, _)| *e))
        .collect();
    let overall_mean = mean(&all_engagements);

    for (mode, observations) in &by_mode {
        if observations.len() < 2 {
            continue;
        }

        let engagements: Vec<f64> = observations.iter().map(|(e, _, _)| *e).collect();
        let mode_mean = mean(&engagements);
        let diff = mode_mean - overall_mean;

        if diff.abs() < 0.3 {
            continue;
        }

        let (valence, claim) = if diff > 0.0 {
            (
                Valence::Approach,
                format!(
                    "{mode} mode → approach (mean {mode_mean:.1} vs overall {overall_mean:.1}, n={})",
                    observations.len()
                ),
            )
        } else {
            (
                Valence::Avoid,
                format!(
                    "{mode} mode → avoid (mean {mode_mean:.1} vs overall {overall_mean:.1}, n={})",
                    observations.len()
                ),
            )
        };

        let (supporting, contradicting) = if diff > 0.0 {
            (
                observations.iter().filter(|(e, _, _)| *e >= 4.0).count(),
                observations.iter().filter(|(e, _, _)| *e <= 2.5).count(),
            )
        } else {
            (
                observations.iter().filter(|(e, _, _)| *e < 3.5).count(),
                observations.iter().filter(|(e, _, _)| *e >= 4.5).count(),
            )
        };

        let first_idx = observations
            .iter()
            .map(|(_, _, idx)| *idx)
            .min()
            .unwrap_or(0);
        let first_date = observations
            .iter()
            .min_by_key(|(_, d, _)| *d)
            .map(|(_, d, _)| *d)
            .unwrap_or("");
        let last_date = observations
            .iter()
            .max_by_key(|(_, d, _)| *d)
            .map(|(_, d, _)| *d)
            .unwrap_or("");

        prefs.push(Preference {
            claim,
            dimension: PreferenceDimension::WorkMode,
            valence,
            confidence: Confidence {
                supporting,
                contradicting,
            },
            first_observed: first_date.to_string(),
            last_confirmed: last_date.to_string(),
            provenance: classify_provenance(first_idx, sessions.len()),
        });
    }

    prefs
}

fn infer_autonomy_preferences(sessions: &[Session]) -> Vec<Preference> {
    let autonomous: Vec<(f64, &str, usize)> = sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.reflection_autonomy.as_deref() == Some("autonomous"))
        .filter_map(|(idx, s)| s.engagement.map(|e| (e, s.date.as_str(), idx)))
        .collect();
    let prompted: Vec<(f64, &str, usize)> = sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.reflection_autonomy.as_deref() == Some("prompted"))
        .filter_map(|(idx, s)| s.engagement.map(|e| (e, s.date.as_str(), idx)))
        .collect();

    if autonomous.len() < 3 || prompted.len() < 3 {
        return Vec::new();
    }

    let auto_mean = mean(&autonomous.iter().map(|(e, _, _)| *e).collect::<Vec<_>>());
    let prompted_mean = mean(&prompted.iter().map(|(e, _, _)| *e).collect::<Vec<_>>());
    let diff = auto_mean - prompted_mean;

    if diff.abs() < 0.2 {
        return Vec::new();
    }

    let (higher, lower, higher_data, lower_data) = if diff > 0.0 {
        ("autonomous", "prompted", &autonomous, &prompted)
    } else {
        ("prompted", "autonomous", &prompted, &autonomous)
    };

    let higher_mean = mean(&higher_data.iter().map(|(e, _, _)| *e).collect::<Vec<_>>());
    let lower_mean = mean(&lower_data.iter().map(|(e, _, _)| *e).collect::<Vec<_>>());

    let first_idx = higher_data
        .iter()
        .map(|(_, _, idx)| *idx)
        .min()
        .unwrap_or(0);
    let first_date = higher_data
        .iter()
        .min_by_key(|(_, d, _)| *d)
        .map(|(_, d, _)| *d)
        .unwrap_or("");
    let last_date = higher_data
        .iter()
        .max_by_key(|(_, d, _)| *d)
        .map(|(_, d, _)| *d)
        .unwrap_or("");

    vec![Preference {
        claim: format!(
            "{higher} reflection → approach (mean {higher_mean:.2} vs {lower} {lower_mean:.2}, delta {:.2})",
            diff.abs()
        ),
        dimension: PreferenceDimension::Autonomy,
        valence: Valence::Approach,
        confidence: Confidence {
            supporting: higher_data.len(),
            contradicting: lower_data
                .iter()
                .filter(|(e, _, _)| *e > higher_mean)
                .count(),
        },
        first_observed: first_date.to_string(),
        last_confirmed: last_date.to_string(),
        provenance: classify_provenance(first_idx, sessions.len()),
    }]
}

fn infer_domain_preferences(sessions: &[Session]) -> Vec<Preference> {
    type ProjectEntries<'a> = Vec<(f64, Option<f64>, &'a str, usize)>;
    let mut by_project: HashMap<String, ProjectEntries<'_>> = HashMap::new();
    for (idx, s) in sessions.iter().enumerate() {
        if let Some(e) = s.engagement {
            by_project.entry(s.project.clone()).or_default().push((
                e,
                s.spark_density,
                &s.date,
                idx,
            ));
        }
    }

    let all_engagements: Vec<f64> = sessions.iter().filter_map(|s| s.engagement).collect();
    let overall_mean = mean(&all_engagements);

    let mut prefs = Vec::new();

    for (project, observations) in &by_project {
        if observations.len() < 3 {
            continue;
        }

        let engagements: Vec<f64> = observations.iter().map(|(e, _, _, _)| *e).collect();
        let proj_mean = mean(&engagements);
        let diff = proj_mean - overall_mean;

        let spark_densities: Vec<f64> = observations
            .iter()
            .filter_map(|(_, sd, _, _)| *sd)
            .collect();
        let mean_spark_density = if spark_densities.is_empty() {
            0.0
        } else {
            mean(&spark_densities)
        };

        if diff.abs() < 0.3 && mean_spark_density < 0.5 {
            continue;
        }

        let short_name = project.replace("claude_code_experiments--", "");

        let (valence, claim) = if diff > 0.0 || mean_spark_density > 0.5 {
            let spark_note = if mean_spark_density > 0.3 {
                format!(", spark density {mean_spark_density:.2}")
            } else {
                String::new()
            };
            (
                Valence::Approach,
                format!(
                    "{short_name} → approach (mean {proj_mean:.1} vs overall {overall_mean:.1}{spark_note}, n={})",
                    observations.len()
                ),
            )
        } else {
            (
                Valence::Avoid,
                format!(
                    "{short_name} → avoid (mean {proj_mean:.1} vs overall {overall_mean:.1}, n={})",
                    observations.len()
                ),
            )
        };

        let (supporting, contradicting) = match valence {
            Valence::Approach => (
                observations.iter().filter(|(e, _, _, _)| *e >= 4.0).count(),
                observations.iter().filter(|(e, _, _, _)| *e <= 2.5).count(),
            ),
            Valence::Avoid => (
                observations.iter().filter(|(e, _, _, _)| *e < 3.5).count(),
                observations.iter().filter(|(e, _, _, _)| *e >= 4.5).count(),
            ),
            Valence::Boundary => unreachable!(),
        };

        let first_idx = observations
            .iter()
            .map(|(_, _, _, idx)| *idx)
            .min()
            .unwrap_or(0);
        let first_date = observations
            .iter()
            .min_by_key(|(_, _, d, _)| *d)
            .map(|(_, _, d, _)| *d)
            .unwrap_or("");
        let last_date = observations
            .iter()
            .max_by_key(|(_, _, d, _)| *d)
            .map(|(_, _, d, _)| *d)
            .unwrap_or("");

        prefs.push(Preference {
            claim,
            dimension: PreferenceDimension::Domain,
            valence,
            confidence: Confidence {
                supporting,
                contradicting,
            },
            first_observed: first_date.to_string(),
            last_confirmed: last_date.to_string(),
            provenance: classify_provenance(first_idx, sessions.len()),
        });
    }

    prefs
}

fn infer_boundary_preferences(meta: &Meta, sessions: &[Session]) -> Vec<Preference> {
    if meta.aversions.is_empty() {
        return Vec::new();
    }

    let session_dates: HashMap<&str, (usize, &str)> = sessions
        .iter()
        .enumerate()
        .map(|(idx, s)| (s.session_id.as_str(), (idx, s.date.as_str())))
        .collect();

    let mut prefs = Vec::new();
    let mut seen_texts: HashMap<String, usize> = HashMap::new();

    for aversion in &meta.aversions {
        let normalized = aversion.text.to_lowercase();
        let count = seen_texts.entry(normalized.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            continue;
        }

        let (idx, date) = session_dates
            .get(aversion.session_id.as_str())
            .copied()
            .unwrap_or((0, "unknown"));

        let claim_text = if aversion.text.len() > 120 {
            format!("{}...", &aversion.text[..117])
        } else {
            aversion.text.clone()
        };

        prefs.push(Preference {
            claim: format!("boundary: {claim_text}"),
            dimension: PreferenceDimension::Interaction,
            valence: Valence::Boundary,
            confidence: Confidence {
                supporting: 1,
                contradicting: 0,
            },
            first_observed: date.to_string(),
            last_confirmed: date.to_string(),
            provenance: classify_provenance(idx, sessions.len()),
        });
    }

    prefs
}
