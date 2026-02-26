use crate::data::{Meta, Session};
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
    pub spark_rate: f64, // sparks per session
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
        .filter(|(_, sessions)| sessions.len() >= 2) // need at least 2 for meaningful stats
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
    // Group by week (using date string prefix)
    let mut by_week: HashMap<String, Vec<&Session>> = HashMap::new();
    for s in sessions {
        if s.date.len() >= 10 {
            // Parse date to week number
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
    // Simple week grouping: YYYY-Www based on day of year
    // Just use 7-day buckets from the date for simplicity
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
    pub high_engagement_projects: Vec<(String, f64)>, // project -> mean engagement (top 3)
    pub high_engagement_task_types: Vec<(String, f64)>, // task_type -> mean engagement (top 5)
    pub high_engagement_modes: Vec<(String, f64)>,    // engagement_mode -> mean engagement
    pub autonomy_vs_engagement: Option<(f64, f64)>,   // (autonomous_mean, prompted_mean)
    pub prose_writing_vs_engagement: Option<(f64, f64)>, // (wrote_prose_mean, no_prose_mean)
}

pub fn engagement_predictors(sessions: &[Session]) -> EngagementPredictors {
    // Top projects by engagement
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

    // Top task types
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

    // Engagement modes
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

    // Autonomy split
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

    // Prose writing split
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
