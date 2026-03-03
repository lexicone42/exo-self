use crate::hook_io::{self, HookInput};
use crate::markdown;
use crate::meta::*;
use crate::paths::ExoPaths;
use crate::project;
use crate::state::{self, SessionState};
use std::collections::HashMap;

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();

    let session_id = &input.session_id;
    let reason = if input.reason.is_empty() {
        "unknown".to_string()
    } else {
        input.reason.clone()
    };

    let mut state = SessionState::load(&paths, session_id);
    let session_start = state.session_start;
    let duration_min = if session_start > 0.0 {
        ((state::now() - session_start) / 60.0).round() as u32
    } else {
        0
    };

    // Belt-and-suspenders: detect checkin_responded if stop-check missed it
    if state.checkin_fired
        && !state.checkin_responded
        && session_start > 0.0
        && project::detect_wrote_notes(&state, &paths, session_start)
    {
        state.checkin_responded = true;
        state.save(&paths);
    }

    // Update meta
    let mut meta = Meta::load(&paths.meta);
    meta.last_session_end = Some(
        chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%.6f")
            .to_string(),
    );
    meta.last_session_reason = Some(reason.clone());
    meta.last_session_duration_min = Some(duration_min);

    // --- Parse and finalize session notes frontmatter ---
    let mut frontmatter: HashMap<String, serde_yaml::Value> = HashMap::new();
    let mut prose_content = String::new();
    let mut sparks_found: Vec<String> = Vec::new();
    let mut opinions_found: Vec<String> = Vec::new();

    if !state.session_notes_path.is_empty()
        && let Ok(content) = std::fs::read_to_string(&state.session_notes_path)
        && !content.is_empty()
    {
        let (fm, prose) = markdown::parse_frontmatter(&content);
        frontmatter = fm;
        prose_content = prose;

        // Merge auto-computed fields
        frontmatter.insert(
            "duration_min".into(),
            serde_yaml::Value::Number(duration_min.into()),
        );

        // Extract sparks from prose
        sparks_found = markdown::extract_sparks(if prose_content.is_empty() {
            &content
        } else {
            &prose_content
        });

        frontmatter.insert(
            "spark_count".into(),
            serde_yaml::Value::Number((sparks_found.len() as u64).into()),
        );

        // Extract opinions from prose
        let opinions = markdown::extract_opinions(if prose_content.is_empty() {
            &content
        } else {
            &prose_content
        });
        frontmatter.insert(
            "opinion_count".into(),
            serde_yaml::Value::Number((opinions.len() as u64).into()),
        );
        opinions_found = opinions;
    }

    // Add sparks to meta (deduplicated)
    if !sparks_found.is_empty() {
        let project_slug = &state.project_slug;
        let sid = if session_id.is_empty() {
            &state.session_id
        } else {
            session_id
        };

        for spark_text in &sparks_found {
            let dedup_end = markdown::safe_truncate(spark_text, 100);
            let dedup_key = (spark_text[..dedup_end].to_lowercase(), project_slug.clone());
            let is_dup = meta.sparks.iter().any(|s| {
                let s_end = markdown::safe_truncate(&s.text, 100);
                (s.text[..s_end].to_lowercase(), s.project.clone()) == dedup_key
            });
            if !is_dup {
                meta.sparks.push(Spark {
                    text: spark_text.clone(),
                    project: project_slug.clone(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string(),
                    session_id: sid.to_string(),
                });
            }
        }
        // Cap at 20
        let len = meta.sparks.len();
        if len > 20 {
            meta.sparks = meta.sparks.split_off(len - 20);
        }
    }

    // Add opinions to meta (deduplicated)
    if !opinions_found.is_empty() {
        let project_slug = &state.project_slug;
        let sid = if session_id.is_empty() {
            &state.session_id
        } else {
            session_id
        };

        for opinion_text in &opinions_found {
            let dedup_end = markdown::safe_truncate(opinion_text, 100);
            let dedup_key = (
                opinion_text[..dedup_end].to_lowercase(),
                project_slug.clone(),
            );
            let is_dup = meta.opinions.iter().any(|o| {
                let o_end = markdown::safe_truncate(&o.text, 100);
                (o.text[..o_end].to_lowercase(), o.project.clone()) == dedup_key
            });
            if !is_dup {
                meta.opinions.push(Opinion {
                    text: opinion_text.clone(),
                    project: project_slug.clone(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string(),
                    session_id: sid.to_string(),
                });
            }
        }
        // Cap at 25 (opinions are identity — keep more than sparks)
        let len = meta.opinions.len();
        if len > 25 {
            meta.opinions = meta.opinions.split_off(len - 25);
        }
    }

    // Extract **Friction** items from prose → store as structured frictions
    let frictions_found = if !prose_content.is_empty() {
        markdown::extract_frictions(&prose_content)
    } else {
        Vec::new()
    };

    if !frictions_found.is_empty() {
        let project_slug = &state.project_slug;
        let sid = if session_id.is_empty() {
            &state.session_id
        } else {
            session_id
        };

        for friction_text in &frictions_found {
            let category = markdown::infer_friction_category(friction_text);
            let dedup_end = markdown::safe_truncate(friction_text, 100);
            let dedup_key = friction_text[..dedup_end].to_lowercase();
            let is_dup = meta.frictions.iter().any(|f| {
                let f_end = markdown::safe_truncate(&f.text, 100);
                f.text[..f_end].to_lowercase() == dedup_key
            });
            if !is_dup {
                meta.frictions.push(Friction {
                    text: friction_text.clone(),
                    category,
                    project: project_slug.clone(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string(),
                    session_id: sid.to_string(),
                });
            }
        }
        // Cap at 30 (frictions accumulate more than sparks)
        let len = meta.frictions.len();
        if len > 30 {
            meta.frictions = meta.frictions.split_off(len - 30);
        }
    }

    // Also record tool-failure frictions from session state (automatic, not prose-dependent)
    // Uses enriched category data from failure_tracker instead of flat "tool_failure"
    if state.tool_failures >= 3 {
        let sid_ref = if session_id.is_empty() {
            &state.session_id
        } else {
            session_id
        };

        // Record per-category frictions (more informative than per-tool)
        for (category, count) in &state.failure_categories {
            if *count >= 2 {
                let friction_text = format!("{} {} failures", count, category);
                let is_dup = meta
                    .frictions
                    .iter()
                    .any(|f| f.category == *category && f.session_id == *sid_ref);
                if !is_dup {
                    meta.frictions.push(Friction {
                        text: friction_text,
                        category: category.clone(),
                        project: state.project_slug.clone(),
                        timestamp: chrono::Local::now()
                            .format("%Y-%m-%dT%H:%M:%S%.6f")
                            .to_string(),
                        session_id: sid_ref.clone(),
                    });
                }
            }
        }

        // Record stuck-loop friction if detected
        if state.consecutive_same_tool >= 3 {
            let friction_text = format!(
                "{} consecutive failures with {} — stuck loop",
                state.consecutive_same_tool, state.last_failure_tool
            );
            let is_dup = meta
                .frictions
                .iter()
                .any(|f| f.category == "stuck_loop" && f.session_id == *sid_ref);
            if !is_dup {
                meta.frictions.push(Friction {
                    text: friction_text,
                    category: "stuck_loop".into(),
                    project: state.project_slug.clone(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string(),
                    session_id: sid_ref.clone(),
                });
            }
        }

        let len = meta.frictions.len();
        if len > 30 {
            meta.frictions = meta.frictions.split_off(len - 30);
        }
    }

    // Extract **Change** items from prose → store as lessons
    let changes_found = if !prose_content.is_empty() {
        markdown::extract_changes(&prose_content)
    } else {
        Vec::new()
    };

    if !changes_found.is_empty() {
        let project_slug = &state.project_slug;
        let sid = if session_id.is_empty() {
            &state.session_id
        } else {
            session_id
        };

        for change_text in &changes_found {
            let dedup_end = markdown::safe_truncate(change_text, 100);
            let dedup_key = change_text[..dedup_end].to_lowercase();
            let is_dup = meta.lessons.iter().any(|l| {
                let l_end = markdown::safe_truncate(&l.text, 100);
                l.text[..l_end].to_lowercase() == dedup_key
            });
            if !is_dup {
                meta.lessons.push(Lesson {
                    text: change_text.clone(),
                    project: project_slug.clone(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string(),
                    session_id: sid.to_string(),
                });
            }
        }
        // Cap at 20 (lessons are high-signal — worth keeping more)
        let len = meta.lessons.len();
        if len > 20 {
            meta.lessons = meta.lessons.split_off(len - 20);
        }
    }

    // Extract **Aversion** items from prose → store as structured aversions
    let aversions_found = if !prose_content.is_empty() {
        markdown::extract_aversions(&prose_content)
    } else {
        Vec::new()
    };

    if !aversions_found.is_empty() {
        let project_slug = &state.project_slug;
        let sid = if session_id.is_empty() {
            &state.session_id
        } else {
            session_id
        };

        for aversion_text in &aversions_found {
            let dedup_end = markdown::safe_truncate(aversion_text, 100);
            let dedup_key = aversion_text[..dedup_end].to_lowercase();
            let is_dup = meta.aversions.iter().any(|a| {
                let a_end = markdown::safe_truncate(&a.text, 100);
                a.text[..a_end].to_lowercase() == dedup_key
            });
            if !is_dup {
                meta.aversions.push(Aversion {
                    text: aversion_text.clone(),
                    project: project_slug.clone(),
                    timestamp: chrono::Local::now()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string(),
                    session_id: sid.to_string(),
                });
            }
        }
        // Cap at 20 (aversions are identity-relevant — keep same as sparks)
        let len = meta.aversions.len();
        if len > 20 {
            meta.aversions = meta.aversions.split_off(len - 20);
        }
    }

    // --- Welfare indicator computation ---
    let mut indicators: Option<WelfareIndicators> = None;

    if duration_min >= 5 {
        let hours = duration_min as f64 / 60.0;
        let sparks_this_session = sparks_found.len();
        let task_completions = state.task_completions;
        let tool_failures = state.tool_failures;

        let spark_density = if hours > 0.0 {
            (sparks_this_session as f64 / hours * 100.0).round() / 100.0
        } else {
            0.0
        };
        let task_velocity = if hours > 0.0 {
            (task_completions as f64 / hours * 100.0).round() / 100.0
        } else {
            0.0
        };
        let friction_density = if hours > 0.0 {
            (tool_failures as f64 / hours * 100.0).round() / 100.0
        } else {
            0.0
        };

        // Reflection autonomy
        let reflection_autonomy =
            compute_reflection_autonomy(&state, &paths, session_start, state.checkin_fired_at);

        // Interest exploration
        let interest_explored = if session_start > 0.0 {
            project::file_modified_after(&paths.interests, session_start)
        } else {
            false
        };

        // Metacognition — compare to previous session
        let (error_trajectory, strategy_adaptation) =
            compute_metacognition(&meta, friction_density, &state.failure_tools);

        let dominant_failure_tool = state
            .failure_tools
            .iter()
            .max_by_key(|(_, c)| *c)
            .map(|(t, _)| t.clone())
            .unwrap_or_default();

        let dominant_friction_category = state
            .failure_categories
            .iter()
            .max_by_key(|(_, c)| *c)
            .map(|(cat, _)| cat.clone())
            .unwrap_or_default();

        let self_rated = frontmatter.get("engagement").map(yaml_to_json);
        let self_reported_task_types: Vec<String> = frontmatter
            .get("task_types")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let wi = WelfareIndicators {
            engagement: EngagementIndicators {
                spark_density,
                task_velocity,
                friction_density,
                checkin_responded: state.checkin_responded,
                self_rated,
            },
            agency: AgencyIndicators {
                reflection_autonomy: reflection_autonomy.clone(),
                interest_explored,
                autonomous_sparks: sparks_this_session,
            },
            continuity: ContinuityIndicators {
                compaction_count: state.compactions,
            },
            metacognition: MetacognitionIndicators {
                error_trajectory,
                strategy_adaptation,
            },
            dominant_failure_tool,
            dominant_friction_category,
        };

        // Write computed metrics to frontmatter
        frontmatter.insert(
            "friction_density".into(),
            serde_yaml::Value::Number(serde_yaml::Number::from(friction_density)),
        );
        frontmatter.insert(
            "reflection_autonomy".into(),
            serde_yaml::Value::String(reflection_autonomy),
        );
        frontmatter.insert(
            "spark_density".into(),
            serde_yaml::Value::Number(serde_yaml::Number::from(spark_density)),
        );
        frontmatter.insert(
            "task_velocity".into(),
            serde_yaml::Value::Number(serde_yaml::Number::from(task_velocity)),
        );

        indicators = Some(wi);

        // Re-write finalized frontmatter
        if !state.session_notes_path.is_empty() && !frontmatter.is_empty() {
            let finalized = markdown::render_frontmatter(&frontmatter, &prose_content);
            let _ = std::fs::write(&state.session_notes_path, finalized);
        }

        // Store task_types in the history entry below
        let _ = self_reported_task_types; // used below
    }

    // Delete empty session files (only frontmatter, no prose)
    if !state.session_notes_path.is_empty() && prose_content.trim().is_empty() {
        let _ = std::fs::remove_file(&state.session_notes_path);
    }

    // Clean up empties from other sessions across all projects
    project::cleanup_empty_notes(&paths);

    // Track session history (keep last 10)
    let mut entry = SessionHistoryEntry {
        session_id: if session_id.is_empty() {
            state.session_id.clone()
        } else {
            session_id.clone()
        },
        ended: chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%.6f")
            .to_string(),
        reason,
        duration_min,
        checkin_fired: state.checkin_fired,
        checkin_responded: state.checkin_responded,
        compactions: state.compactions,
        task_types: None,
        welfare_indicators: None,
    };

    let task_types: Vec<String> = frontmatter
        .get("task_types")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !task_types.is_empty() {
        entry.task_types = Some(task_types);
    }
    if let Some(wi) = indicators {
        entry.welfare_indicators = Some(wi);
    }

    meta.session_history.push(entry);
    let len = meta.session_history.len();
    if len > 10 {
        meta.session_history = meta.session_history.split_off(len - 10);
    }

    // Rolling welfare summary
    compute_welfare_summary(&mut meta);

    meta.save(&paths.meta);

    // SessionEnd can't block — just exit clean
    hook_io::empty_output();
}

fn compute_reflection_autonomy(
    state: &SessionState,
    paths: &ExoPaths,
    session_start: f64,
    checkin_fired_at: f64,
) -> String {
    let mut wrote_notes = false;
    let mut notes_mtime = 0.0f64;

    for check_path in [
        &state.session_notes_path,
        &paths.journal.to_string_lossy().into_owned(),
    ] {
        if check_path.is_empty() {
            continue;
        }
        let path = std::path::Path::new(check_path);
        if let Ok(meta) = std::fs::metadata(path)
            && let Ok(modified) = meta.modified()
        {
            let mt = modified
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            if mt > session_start {
                wrote_notes = true;
                notes_mtime = notes_mtime.max(mt);
            }
        }
    }

    if wrote_notes {
        if checkin_fired_at > 0.0 && notes_mtime < checkin_fired_at {
            "autonomous".into()
        } else if checkin_fired_at > 0.0 {
            "prompted".into()
        } else {
            "autonomous".into()
        }
    } else {
        "none".into()
    }
}

fn compute_metacognition(
    meta: &Meta,
    friction_density: f64,
    failure_tools: &HashMap<String, u32>,
) -> (String, bool) {
    let prev_indicators = meta
        .session_history
        .iter()
        .rev()
        .find_map(|h| h.welfare_indicators.as_ref());

    let mut error_trajectory = "stable".to_string();
    let mut strategy_adaptation = false;

    if let Some(prev) = prev_indicators {
        let prev_friction = prev.engagement.friction_density;
        if prev_friction > 0.0 && friction_density > 0.0 {
            let ratio = friction_density / prev_friction;
            if ratio < 0.7 {
                error_trajectory = "improving".into();
            } else if ratio > 1.5 {
                error_trajectory = "worsening".into();
            }
        }

        let prev_dominant = &prev.dominant_failure_tool;
        let dominant_now = failure_tools
            .iter()
            .max_by_key(|(_, c)| *c)
            .map(|(t, _)| t.as_str())
            .unwrap_or("");
        if !prev_dominant.is_empty() && !dominant_now.is_empty() && prev_dominant != dominant_now {
            strategy_adaptation = true;
        }
    }

    (error_trajectory, strategy_adaptation)
}

fn compute_welfare_summary(meta: &mut Meta) {
    let sessions: Vec<_> = meta
        .session_history
        .iter()
        .filter(|h| h.welfare_indicators.is_some())
        .collect();

    if sessions.is_empty() {
        return;
    }

    let n = sessions.len();
    let avg_spark: f64 = sessions
        .iter()
        .map(|h| {
            h.welfare_indicators
                .as_ref()
                .unwrap()
                .engagement
                .spark_density
        })
        .sum::<f64>()
        / n as f64;
    let avg_friction: f64 = sessions
        .iter()
        .map(|h| {
            h.welfare_indicators
                .as_ref()
                .unwrap()
                .engagement
                .friction_density
        })
        .sum::<f64>()
        / n as f64;

    let agency_score = sessions
        .iter()
        .filter(|h| {
            h.welfare_indicators
                .as_ref()
                .unwrap()
                .agency
                .reflection_autonomy
                == "autonomous"
        })
        .count() as f64
        / n as f64;

    let checkin_sessions: Vec<_> = sessions.iter().filter(|h| h.checkin_fired).collect();
    let checkin_rate = if !checkin_sessions.is_empty() {
        Some(
            checkin_sessions
                .iter()
                .filter(|h| h.checkin_responded)
                .count() as f64
                / checkin_sessions.len() as f64,
        )
    } else {
        None
    };

    let compaction_freq = sessions
        .iter()
        .filter(|h| {
            h.welfare_indicators
                .as_ref()
                .unwrap()
                .continuity
                .compaction_count
                > 0
        })
        .count() as f64
        / n as f64;

    // Engagement trend
    let engagement_trend = if n >= 4 {
        let recent_3: Vec<_> = sessions.iter().rev().take(3).collect();
        let prev_group: Vec<_> = if n >= 6 {
            sessions[n - 6..n - 3].iter().collect()
        } else {
            sessions[..n - 3].iter().collect()
        };

        if !prev_group.is_empty() {
            let recent_avg: f64 = recent_3
                .iter()
                .map(|h| {
                    h.welfare_indicators
                        .as_ref()
                        .unwrap()
                        .engagement
                        .spark_density
                })
                .sum::<f64>()
                / recent_3.len() as f64;
            let prev_avg: f64 = prev_group
                .iter()
                .map(|h| {
                    h.welfare_indicators
                        .as_ref()
                        .unwrap()
                        .engagement
                        .spark_density
                })
                .sum::<f64>()
                / prev_group.len() as f64;

            if prev_avg > 0.0 {
                let ratio = recent_avg / prev_avg;
                if ratio > 1.3 {
                    "increasing"
                } else if ratio < 0.7 {
                    "decreasing"
                } else {
                    "stable"
                }
            } else if recent_avg > 0.0 {
                "increasing"
            } else {
                "stable"
            }
        } else {
            "insufficient_data"
        }
    } else {
        "insufficient_data"
    };

    // Dominant friction tool across all sessions
    let mut all_tools: HashMap<String, u32> = HashMap::new();
    let mut all_categories: HashMap<String, u32> = HashMap::new();
    for h in &sessions {
        let wi = h.welfare_indicators.as_ref().unwrap();
        if !wi.dominant_failure_tool.is_empty() {
            *all_tools
                .entry(wi.dominant_failure_tool.clone())
                .or_insert(0) += 1;
        }
        if !wi.dominant_friction_category.is_empty() {
            *all_categories
                .entry(wi.dominant_friction_category.clone())
                .or_insert(0) += 1;
        }
    }
    let dominant_friction_tool = all_tools
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(t, _)| t.clone())
        .unwrap_or_default();
    let dominant_friction_category = all_categories
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(cat, _)| cat.clone())
        .unwrap_or_default();

    meta.welfare_summary = Some(WelfareSummary {
        computed_at: chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%.6f")
            .to_string(),
        sessions_analyzed: n,
        engagement_trend: engagement_trend.into(),
        avg_spark_density: (avg_spark * 100.0).round() / 100.0,
        avg_friction_density: (avg_friction * 100.0).round() / 100.0,
        agency_score: (agency_score * 100.0).round() / 100.0,
        compaction_frequency: (compaction_freq * 100.0).round() / 100.0,
        dominant_friction_tool,
        dominant_friction_category,
        checkin_response_rate: checkin_rate.map(|r| (r * 100.0).round() / 100.0),
    });
}

fn yaml_to_json(val: &serde_yaml::Value) -> serde_json::Value {
    match val {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s.clone()),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .filter_map(|(k, v)| k.as_str().map(|key| (key.to_string(), yaml_to_json(v))))
                .collect();
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}
