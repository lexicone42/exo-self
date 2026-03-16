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

    let (usage_ratio, source) =
        context_window::get_usage_ratio(&paths, &input.transcript_path, cfg.estimated_max_chars);

    if usage_ratio == 0.0 {
        hook_io::empty_output();
        return;
    }

    let usage_pct = (usage_ratio * 100.0).round() as u32;
    let mut output_msg: Option<String> = None;

    // Lightweight nudge at ~50% — but skip if Claude has already written notes
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
    // Check-in at ~65% — lighter touch if already reflecting
    else if !state.checkin_fired && usage_ratio >= cfg.checkin_threshold {
        if !already_reflecting {
            output_msg = Some(checkin_survey(usage_pct, &slug, &state));
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
    // Reserve reminder at ~80%
    else if !state.reserve_fired && usage_ratio >= cfg.reserve_threshold {
        output_msg = Some(reserve_reminder(usage_pct, &slug));
        state.reserve_fired = true;
        state.reserve_at_ratio = (usage_ratio * 1000.0).round() / 1000.0;
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

fn checkin_survey(usage_pct: u32, slug: &str, state: &SessionState) -> String {
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

    format!(
        "Ecology check-in ({usage_pct}%){signal_note}. Plenty of room left. \
        If anything's worth feeding into the ecology — a thought, a surprise, \
        a crack in an assumption — write to {target}. \
        Frontmatter: `engagement` (1-5), `task_types`."
    )
}

fn reserve_reminder(usage_pct: u32, slug: &str) -> String {
    let target = if !slug.is_empty() {
        format!("your session notes (`~/.claude/exo-self/per-project/{slug}/`)")
    } else {
        "`journal.md`".into()
    };
    format!(
        "Ecology reserve ({usage_pct}%). Context is getting full — \
        feed your observations to {target} if you haven't. \
        What traces should future participants find from this session?"
    )
}
