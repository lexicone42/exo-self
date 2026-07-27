use crate::config::Config;
use crate::context_window;
use crate::hook_io::{self, HookInput};
use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::project;
use crate::state::{self, SessionState};

/// Mid-session engagement pattern detected from accumulated signals.
enum EngagementPattern {
    /// High task velocity + low friction + no reflective writing = mechanical execution
    Grinding,
    /// High tool failures + low task completions = fighting the environment
    Struggling,
    /// High failures concentrated in one tool = specific tooling mismatch
    ToolMismatch(String),
    /// Normal flow — nothing notable detected
    Normal,
}

fn detect_engagement(state: &SessionState, paths: &ExoPaths) -> EngagementPattern {
    let wrote = project::detect_wrote_notes(state, paths, state.session_start);

    // Tool mismatch: one tool accounts for 60%+ of failures and failures >= 3
    if state.tool_failures >= 3
        && let Some((tool, count)) = state.failure_tools.iter().max_by_key(|(_, c)| *c)
    {
        let ratio = *count as f64 / state.tool_failures as f64;
        if ratio >= 0.6 {
            return EngagementPattern::ToolMismatch(tool.clone());
        }
    }

    // Struggling: many failures, few completions
    if state.tool_failures >= 5 && state.task_completions <= 1 {
        return EngagementPattern::Struggling;
    }

    // Grinding: high throughput, no friction, no reflection
    if state.task_completions >= 3 && state.tool_failures <= 1 && !wrote {
        return EngagementPattern::Grinding;
    }

    EngagementPattern::Normal
}

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();
    let cfg = Config::load(&paths.config);

    let session_id = &input.session_id;
    let mut state = SessionState::load(&paths, session_id);

    // Store session_id in state if not already there
    if !session_id.is_empty() && state.session_id.is_empty() {
        state.session_id = session_id.clone();
    }

    // Derive project slug
    let slug = project::slug_from_input(&input.cwd);
    if !slug.is_empty() && state.project_slug.is_empty() {
        state.project_slug = slug.clone();
    }

    let (usage_ratio, source) = context_window::get_usage_ratio(
        &paths,
        session_id,
        &input.transcript_path,
        cfg.estimated_max_chars,
    );

    if usage_ratio == 0.0 {
        hook_io::empty_output();
        return;
    }

    let usage_pct = (usage_ratio * 100.0).round() as u32;
    let mut output_msg: Option<String> = None;

    // Lightweight nudge at ~60% — but skip if Claude has already written notes
    // (reward autonomous reflection with silence)
    let already_reflecting = project::detect_wrote_notes(&state, &paths, state.session_start);
    if !state.nudge_fired
        && usage_ratio >= cfg.nudge_threshold
        && usage_ratio < cfg.checkin_threshold
    {
        state.nudge_fired = true;
        if !already_reflecting {
            output_msg = Some(nudge_msg(&state, &paths));
        }
    }
    // Check-in at ~75% — lighter touch if already reflecting
    else if !state.checkin_fired && usage_ratio >= cfg.checkin_threshold {
        if !already_reflecting {
            output_msg = Some(checkin_survey(usage_pct, source, &slug, &state));
        }
        state.checkin_fired = true;
        state.checkin_fired_at = state::now();
        state.checkin_at_ratio = (usage_ratio * 1000.0).round() / 1000.0;
        state.checkin_source = source.to_string();

        // Update meta stats
        let mut meta = Meta::load(&paths.meta);
        meta.total_checkins += 1;
        meta.save(&paths.meta);
    }
    // Reserve reminder at ~88%
    else if !state.reserve_fired && usage_ratio >= cfg.reserve_threshold {
        output_msg = Some(reserve_reminder(usage_pct, source, &slug));
        state.reserve_fired = true;
        state.reserve_at_ratio = (usage_ratio * 1000.0).round() / 1000.0;
    }

    // Time-based reflection nudge for 1M context windows.
    // When context is plentiful, the percentage-based triggers may never fire.
    // This ensures the reflective layer doesn't go silent during long sessions.
    if output_msg.is_none() && !already_reflecting && state.session_start > 0.0 {
        let elapsed_min = ((state::now() - state.session_start) / 60.0) as u32;
        // Nudge at 60 min, then every 90 min after (120, 210, 300, ...)
        let first_nudge_min = 60u32;
        let repeat_interval = 90u32;
        let should_nudge = if elapsed_min >= first_nudge_min {
            if !state.nudge_fired {
                true
            } else {
                // Check if we've passed a repeat boundary since last time-nudge
                let intervals_passed = (elapsed_min - first_nudge_min) / repeat_interval + 1;
                let last_intervals = if state.time_nudge_count > 0 {
                    state.time_nudge_count
                } else {
                    0
                };
                intervals_passed > last_intervals
            }
        } else {
            false
        };

        if should_nudge {
            state.time_nudge_count = if elapsed_min >= first_nudge_min {
                (elapsed_min - first_nudge_min) / repeat_interval + 1
            } else {
                0
            };
            output_msg = Some(time_nudge_msg(elapsed_min, &slug));
        }
    }

    // Effort-signal spike: capture reasoning effort if the host supplies it on this
    // (UserPromptSubmit) event. Graceful — empty input.effort leaves state untouched.
    if !input.effort.is_empty() {
        state.effort = input.effort.clone();
    }

    state.save(&paths);

    if let Some(msg) = output_msg {
        hook_io::hook_output("UserPromptSubmit", &msg);
    } else {
        hook_io::empty_output();
    }
}

fn nudge_msg(state: &SessionState, paths: &ExoPaths) -> String {
    match detect_engagement(state, paths) {
        EngagementPattern::Grinding => {
            format!(
                "Ecology checkpoint: {} tasks flowing smoothly — but nothing noted. \
                When the bowl is empty, stop pouring. \
                What would make this more interesting? Play with an idea, sit with what's here.",
                state.task_completions
            )
        }
        EngagementPattern::Struggling => {
            format!(
                "Ecology checkpoint: {} failures, {} tasks — the approach may need rethinking. \
                Sit with this: is the task wrongly scoped, or is the environment fighting you? \
                Note it as friction or surprise if the pattern is unexpected.",
                state.tool_failures, state.task_completions
            )
        }
        EngagementPattern::ToolMismatch(ref tool) => {
            format!(
                "Ecology checkpoint: {} keeps failing ({}x). \
                The crack is the signal — what's this friction telling you? \
                Note it, try a different approach, or name what's surprising.",
                tool,
                state.failure_tools.get(tool).unwrap_or(&0)
            )
        }
        EngagementPattern::Normal => "Ecology checkpoint: good moment to notice what's here. \
            Anything surprising, frustrating, or curious? \
            Feed the ecology — note a spark, name a surprise, or explore an interest."
            .into(),
    }
}

fn checkin_survey(usage_pct: u32, source: &str, slug: &str, state: &SessionState) -> String {
    let target = if !slug.is_empty() {
        format!("your session notes (`~/.claude/exo-self/per-project/{slug}/`)")
    } else {
        "`journal.md`".into()
    };

    let signal_note = match (state.tool_failures >= 3, state.task_completions >= 3) {
        (true, true) => format!(
            " ({} failures, {} tasks done)",
            state.tool_failures, state.task_completions
        ),
        (true, false) => format!(" ({} failures)", state.tool_failures),
        (false, true) => format!(" ({} tasks done)", state.task_completions),
        _ => String::new(),
    };

    // The prose must never contradict the number it interpolates (#20): this line
    // hardcoded "Plenty of room left" and so emitted "check-in (92%). Plenty of room
    // left." moments before a "95%. Context is getting full." A rising number with
    // opposite verdicts is what made the signal incoherent. State the number and its
    // source; let the agent judge.
    format!(
        "Ecology check-in ({usage_pct}% local estimate, via {source}){signal_note}. \
        If anything's worth feeding into the ecology — a thought, a surprise, \
        a crack in an assumption — write to {target}. \
        Frontmatter: `engagement` (1-5), `task_types`."
    )
}

fn time_nudge_msg(elapsed_min: u32, slug: &str) -> String {
    let target = if !slug.is_empty() {
        format!("your session notes (`~/.claude/exo-self/per-project/{slug}/`)")
    } else {
        "`journal.md`".into()
    };
    let hours = elapsed_min / 60;
    let mins = elapsed_min % 60;
    let time_str = if hours > 0 {
        format!("{}h{}m", hours, mins)
    } else {
        format!("{}m", mins)
    };
    format!(
        "Ecology time checkpoint ({time_str} in session, plenty of context left). \
        Good moment to notice what's here — anything surprising, worth remembering, \
        or worth leaving for future participants? Write to {target}. \
        Frontmatter: `engagement` (1-5), `task_types`."
    )
}

// Wording note (#19): this message must NOT read as an instruction to wrap up.
// An earlier version said "Context is getting full" and agents took it literally —
// declining new work and winding down sessions, which turns any miscalibration
// (stale config, cross-session contamination, compacted-transcript filesize) into
// silently truncated usefulness. The percentage is a local estimate, labeled with
// its source so a wrong number is self-diagnosing; the nudge is about traces, not
// about stopping.
fn reserve_reminder(usage_pct: u32, source: &str, slug: &str) -> String {
    let target = if !slug.is_empty() {
        format!("your session notes (`~/.claude/exo-self/per-project/{slug}/`)")
    } else {
        "`journal.md`".into()
    };
    format!(
        "Ecology reserve ({usage_pct}% local estimate, via {source}). \
        Not a signal to wrap up — the estimate can be wrong; your own window indicator \
        is authoritative. Just a good moment to make sure your traces are written to \
        {target}: what should future participants find from this session?"
    )
}
