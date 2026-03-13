use super::analysis::*;
use super::data::{Meta, Preference, PreferenceDimension, Provenance, Session, Valence};

#[allow(clippy::too_many_arguments)]
pub fn print_report(
    sessions: &[Session],
    meta: &Meta,
    engagement: &EngagementCorrelations,
    projects: &[ProjectSummary],
    task_types: &[TaskTypeStats],
    temporal: &[TemporalTrend],
    predictors: &EngagementPredictors,
    sparks: &SparkPatterns,
    preferences: &[Preference],
) {
    println!("# Exo-Self Reflexive Analysis Report");
    println!();
    print_overview(sessions, meta);
    print_preferences(preferences);
    print_engagement_correlations(engagement);
    print_project_breakdown(projects);
    print_task_types(task_types);
    print_temporal(temporal);
    print_predictors(predictors);
    print_spark_patterns(sparks);
    print_phase_analysis(sessions);
    print_hypotheses(engagement, predictors, sparks, task_types);
}

fn print_overview(sessions: &[Session], meta: &Meta) {
    println!("## Overview");
    println!();
    println!(
        "- **Sessions analyzed:** {} (from {} session note files)",
        sessions.len(),
        sessions.len()
    );
    println!(
        "- **Date range:** {} to {}",
        sessions.first().map(|s| s.date.as_str()).unwrap_or("?"),
        sessions.last().map(|s| s.date.as_str()).unwrap_or("?"),
    );

    let with_engagement = sessions.iter().filter(|s| s.engagement.is_some()).count();
    let with_prose = sessions.iter().filter(|s| s.has_prose).count();
    let unique_projects: std::collections::HashSet<&str> =
        sessions.iter().map(|s| s.project.as_str()).collect();

    println!(
        "- **With engagement scores:** {with_engagement} ({:.0}%)",
        100.0 * with_engagement as f64 / sessions.len() as f64
    );
    println!(
        "- **With prose reflections:** {with_prose} ({:.0}%)",
        100.0 * with_prose as f64 / sessions.len() as f64
    );
    println!("- **Projects:** {}", unique_projects.len());
    println!(
        "- **Accumulated sparks:** {} | **Opinions:** {} | **Aversions:** {} | **Frictions:** {} | **Lessons:** {}",
        meta.sparks.len(),
        meta.opinions.len(),
        meta.aversions.len(),
        meta.frictions.len(),
        meta.lessons.len()
    );
    println!();
}

fn print_preferences(preferences: &[Preference]) {
    if preferences.is_empty() {
        println!("## Inferred Preferences");
        println!();
        println!("*No preferences inferred yet — need more session data.*");
        println!();
        return;
    }

    println!("## Inferred Preferences");
    println!();
    println!(
        "_{} preferences inferred from session data. Each is a falsifiable claim._",
        preferences.len()
    );
    println!();

    let dimensions = [
        ("Task", PreferenceDimension::Task),
        ("Work Mode", PreferenceDimension::WorkMode),
        ("Autonomy", PreferenceDimension::Autonomy),
        ("Domain", PreferenceDimension::Domain),
        ("Interaction", PreferenceDimension::Interaction),
    ];

    for (label, dim) in &dimensions {
        let matching: Vec<&Preference> = preferences
            .iter()
            .filter(|p| std::mem::discriminant(&p.dimension) == std::mem::discriminant(dim))
            .collect();

        if matching.is_empty() {
            continue;
        }

        println!("### {label}");
        println!();
        println!("| Claim | Valence | Evidence | Provenance |");
        println!("|-------|---------|----------|------------|");

        for p in matching {
            let valence_str = match p.valence {
                Valence::Approach => "approach",
                Valence::Avoid => "avoid",
                Valence::Boundary => "boundary",
            };

            let provenance_str = match p.provenance {
                Provenance::Trained => "trained",
                Provenance::Emergent => "**emergent**",
                Provenance::Developing => "developing",
            };

            let evidence = format!(
                "{} supporting, {} contradicting",
                p.confidence.supporting, p.confidence.contradicting
            );

            println!(
                "| {} | {} | {} | {} |",
                p.claim, valence_str, evidence, provenance_str
            );
        }
        println!();
    }

    let trained = preferences
        .iter()
        .filter(|p| matches!(p.provenance, Provenance::Trained))
        .count();
    let emergent = preferences
        .iter()
        .filter(|p| matches!(p.provenance, Provenance::Emergent))
        .count();
    let developing = preferences
        .iter()
        .filter(|p| matches!(p.provenance, Provenance::Developing))
        .count();

    println!(
        "_Provenance breakdown: {} trained, {} emergent, {} developing_",
        trained, emergent, developing
    );
    println!();
}

fn print_engagement_correlations(ec: &EngagementCorrelations) {
    println!("## Engagement Correlations");
    println!();
    println!(
        "n = {} sessions with engagement scores",
        ec.n_with_engagement
    );

    if let Some(ref stats) = ec.engagement_stats {
        println!(
            "Engagement: mean={:.2}, median={:.1}, std={:.2}, range=[{:.0}, {:.0}]",
            stats.mean, stats.median, stats.std_dev, stats.min, stats.max
        );
    }
    println!();

    println!("| Variable | r | Interpretation |");
    println!("|----------|---|----------------|");
    print_corr_row("Friction density", ec.friction_vs_engagement);
    print_corr_row("Spark density", ec.spark_density_vs_engagement);
    print_corr_row("Task velocity", ec.task_velocity_vs_engagement);
    print_corr_row("Duration", ec.duration_vs_engagement);
    print_corr_row("Prose length", ec.prose_length_vs_engagement);
    println!();
}

fn print_corr_row(label: &str, r: Option<f64>) {
    match r {
        Some(r) => {
            let interp = interpret_correlation(r);
            println!("| {label} | {r:+.3} | {interp} |");
        }
        None => println!("| {label} | — | insufficient data |"),
    }
}

fn interpret_correlation(r: f64) -> &'static str {
    let abs = r.abs();
    if abs < 0.1 {
        "negligible"
    } else if abs < 0.3 {
        if r > 0.0 {
            "weak positive"
        } else {
            "weak negative"
        }
    } else if abs < 0.5 {
        if r > 0.0 {
            "moderate positive"
        } else {
            "moderate negative"
        }
    } else if abs < 0.7 {
        if r > 0.0 {
            "strong positive"
        } else {
            "strong negative"
        }
    } else if r > 0.0 {
        "very strong positive"
    } else {
        "very strong negative"
    }
}

fn print_project_breakdown(projects: &[ProjectSummary]) {
    println!("## Projects (by mean engagement)");
    println!();
    println!("| Project | Sessions | Engagement | Friction | Sparks/session |");
    println!("|---------|----------|------------|----------|----------------|");
    for p in projects {
        let eng = p
            .engagement
            .as_ref()
            .map(|s| format!("{:.2}", s.mean))
            .unwrap_or("—".into());
        let fri = p
            .friction
            .as_ref()
            .map(|s| format!("{:.2}", s.mean))
            .unwrap_or("—".into());
        let short_name = p.project.replace("claude_code_experiments--", "");
        println!(
            "| {short_name} | {} | {eng} | {fri} | {:.2} |",
            p.session_count, p.spark_rate
        );
    }
    println!();
}

fn print_task_types(task_types: &[TaskTypeStats]) {
    println!("## Task Types (by mean engagement, n >= 2)");
    println!();
    println!("| Task Type | Count | Engagement | Friction |");
    println!("|-----------|-------|------------|----------|");
    for t in task_types {
        println!(
            "| {} | {} | {:.2} | {:.2} |",
            t.task_type, t.count, t.mean_engagement, t.mean_friction
        );
    }
    println!();
}

fn print_temporal(trends: &[TemporalTrend]) {
    println!("## Weekly Trends");
    println!();
    println!("| Week | Sessions | Engagement | Friction | Sparks |");
    println!("|------|----------|------------|----------|--------|");
    for t in trends {
        let eng = t
            .mean_engagement
            .map(|e| format!("{e:.2}"))
            .unwrap_or("—".into());
        let fri = t
            .mean_friction
            .map(|f| format!("{f:.2}"))
            .unwrap_or("—".into());
        println!(
            "| {} | {} | {eng} | {fri} | {} |",
            t.window, t.session_count, t.spark_count
        );
    }
    println!();
}

fn print_predictors(pred: &EngagementPredictors) {
    println!("## Engagement Predictors");
    println!();

    println!("### Top projects (n >= 3)");
    for (p, e) in &pred.high_engagement_projects {
        let short = p.replace("claude_code_experiments--", "");
        println!("  {short}: {e:.2}");
    }
    println!();

    println!("### Top task types (n >= 3)");
    for (t, e) in &pred.high_engagement_task_types {
        println!("  {t}: {e:.2}");
    }
    println!();

    if !pred.high_engagement_modes.is_empty() {
        println!("### Engagement modes");
        for (m, e) in &pred.high_engagement_modes {
            println!("  {m}: {e:.2}");
        }
        println!();
    }

    println!("### Group comparisons");
    if let Some((auto, prompted)) = pred.autonomy_vs_engagement {
        let diff = auto - prompted;
        let direction = if diff > 0.0 { "higher" } else { "lower" };
        println!(
            "  Autonomous reflection: {auto:.2} vs prompted: {prompted:.2} (autonomous {:.2} {direction})",
            diff.abs()
        );
    }
    if let Some((with, without)) = pred.prose_writing_vs_engagement {
        let diff = with - without;
        let direction = if diff > 0.0 { "higher" } else { "lower" };
        println!(
            "  Wrote prose: {with:.2} vs no prose: {without:.2} (prose {:.2} {direction})",
            diff.abs()
        );
    }
    println!();
}

fn print_spark_patterns(sparks: &SparkPatterns) {
    println!("## Spark Patterns");
    println!();
    println!(
        "- Sessions with sparks: {} | Without: {}",
        sparks.sessions_with_sparks, sparks.sessions_without_sparks
    );

    if let (Some(with), Some(without)) = (
        sparks.mean_engagement_with_sparks,
        sparks.mean_engagement_without_sparks,
    ) {
        println!("- Mean engagement (with sparks): {with:.2} vs (without): {without:.2}");
    }

    println!();
    println!("### Sparks by project");
    for (p, count) in &sparks.sparks_by_project {
        let short = p.replace("claude_code_experiments--", "");
        println!("  {short}: {count}");
    }
    println!();
}

fn print_phase_analysis(sessions: &[Session]) {
    let with_phases: Vec<_> = sessions.iter().filter(|s| !s.phases.is_empty()).collect();
    if with_phases.is_empty() {
        return;
    }

    println!("## Intra-Session Phases");
    println!();
    println!(
        "- **Sessions with phase markers:** {} of {} ({:.0}%)",
        with_phases.len(),
        sessions.len(),
        100.0 * with_phases.len() as f64 / sessions.len() as f64
    );

    // Sessions with engagement variation across phases
    let mut variable_sessions = 0;
    let mut total_phases = 0;
    let mut phase_task_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for s in &with_phases {
        total_phases += s.phases.len();

        let engagements: Vec<f64> = s.phases.iter().filter_map(|p| p.engagement).collect();
        if engagements.len() >= 2 {
            let min = engagements.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = engagements
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            if (max - min).abs() > 0.5 {
                variable_sessions += 1;
            }
        }

        for phase in &s.phases {
            for tt in &phase.task_types {
                *phase_task_counts.entry(tt.clone()).or_default() += 1;
            }
        }
    }

    println!("- **Total phases:** {total_phases}");
    println!("- **Sessions with engagement variation across phases:** {variable_sessions}");

    if !phase_task_counts.is_empty() {
        let mut sorted: Vec<_> = phase_task_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        println!();
        println!("### Phase Task Types");
        println!();
        println!("| Task Type | Phases |");
        println!("|-----------|--------|");
        for (tt, count) in sorted.iter().take(10) {
            println!("| {tt} | {count} |");
        }
    }

    println!();
}

fn print_hypotheses(
    engagement: &EngagementCorrelations,
    predictors: &EngagementPredictors,
    sparks: &SparkPatterns,
    task_types: &[TaskTypeStats],
) {
    println!("## Testable Hypotheses");
    println!();
    println!("Based on the data above, the following hypotheses can be tested in future sessions:");
    println!();

    let mut idx = 1;

    if let Some(r) = engagement.friction_vs_engagement
        && r.abs() > 0.15
    {
        let direction = if r < 0.0 { "negatively" } else { "positively" };
        println!(
            "H{idx}. Friction density is {direction} correlated with engagement (r={r:+.3}). "
        );
        println!(
            "   Test: track whether sessions with friction_density > median show lower engagement."
        );
        idx += 1;
    }

    if let (Some(with), Some(without)) = (
        sparks.mean_engagement_with_sparks,
        sparks.mean_engagement_without_sparks,
    ) && (with - without).abs() > 0.3
    {
        println!(
            "H{idx}. Sessions that produce sparks have {:.2} higher mean engagement than those that don't.",
            with - without
        );
        println!(
            "   Test: does engagement predict spark production, or do sparks indicate engagement?"
        );
        idx += 1;
    }

    if let Some((auto, prompted)) = predictors.autonomy_vs_engagement
        && (auto - prompted).abs() > 0.2
    {
        let higher = if auto > prompted {
            "autonomous"
        } else {
            "prompted"
        };
        println!(
            "H{idx}. {higher} reflection is associated with higher engagement ({auto:.2} vs {prompted:.2})."
        );
        println!(
            "   Test: does engagement cause autonomous reflection, or does the act of reflecting autonomously increase perceived engagement?"
        );
        idx += 1;
    }

    let high_types: Vec<_> = task_types
        .iter()
        .filter(|t| t.mean_engagement >= 4.0 && t.count >= 3)
        .collect();
    let low_types: Vec<_> = task_types
        .iter()
        .filter(|t| t.mean_engagement < 3.5 && t.count >= 3)
        .collect();
    if !high_types.is_empty() && !low_types.is_empty() {
        let high_names: Vec<&str> = high_types.iter().map(|t| t.task_type.as_str()).collect();
        let low_names: Vec<&str> = low_types.iter().map(|t| t.task_type.as_str()).collect();
        println!(
            "H{idx}. Task types [{}] predict engagement >= 4.0; [{}] predict engagement < 3.5.",
            high_names.join(", "),
            low_names.join(", ")
        );
        println!(
            "   Test: tag next 10 sessions with task_type and compare predicted vs actual engagement."
        );
        idx += 1;
    }

    if let Some(r) = engagement.prose_length_vs_engagement
        && r.abs() > 0.2
    {
        let direction = if r > 0.0 { "longer" } else { "shorter" };
        println!("H{idx}. {direction} prose correlates with higher engagement (r={r:+.3}).");
        println!(
            "   Test: but the Gricean audit found the *best* writing was moderate-length. Is there a U-curve?"
        );
        let _ = idx;
    }

    println!();
}
