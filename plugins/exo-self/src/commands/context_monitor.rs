use crate::config::Config;
use crate::context_window;
use crate::hook_io::{self, HookInput};
use crate::meta::Meta;
use crate::paths::ExoPaths;
use crate::project;
use crate::state::{self, SessionState};

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

    // Lightweight nudge at ~40%
    if !state.nudge_fired
        && usage_ratio >= cfg.nudge_threshold
        && usage_ratio < cfg.checkin_threshold
    {
        output_msg = Some(nudge_msg(&state));
        state.nudge_fired = true;
    }
    // Check-in survey at ~60%
    else if !state.checkin_fired && usage_ratio >= cfg.checkin_threshold {
        output_msg = Some(checkin_survey(usage_pct, &slug, &state));
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

fn nudge_msg(state: &SessionState) -> String {
    if state.tool_failures >= 3 {
        let top_tool = state
            .failure_tools
            .iter()
            .max_by_key(|(_, c)| *c)
            .map(|(t, _)| t.as_str())
            .unwrap_or("tools");
        format!(
            "Exo-self ~40%: {} tool failures ({}) — worth a sentence in your session notes if it's frustrating.",
            state.tool_failures, top_tool
        )
    } else {
        "Exo-self ~40%: anything frustrating, satisfying, or curious? Explore an interest, note a spark, or just think out loud.".into()
    }
}

fn checkin_survey(usage_pct: u32, slug: &str, state: &SessionState) -> String {
    let target = if !slug.is_empty() {
        format!("your session notes file (`~/.claude/exo-self/per-project/{slug}/`)")
    } else {
        "`journal.md`".into()
    };

    let failures = state.tool_failures;
    let tasks = state.task_completions;

    let failure_note = if failures >= 3 {
        format!(" ({} tool failures)", failures)
    } else {
        String::new()
    };
    let task_note = if tasks >= 3 {
        format!(" ({} tasks done)", tasks)
    } else {
        String::new()
    };

    format!(
        "Exo-self check-in ({usage_pct}%). Write to {target}: \
        **Friction** — what's been hard?{failure_note} \
        **Spark** — what's been satisfying?{task_note} \
        **Change** — one thing to improve? \
        Also fill in frontmatter: `engagement` (1-5) and `task_types`."
    )
}

fn reserve_reminder(usage_pct: u32, slug: &str) -> String {
    let target = if !slug.is_empty() {
        format!("your session notes (`~/.claude/exo-self/per-project/{slug}/`)")
    } else {
        "`journal.md`".into()
    };
    format!(
        "Exo-self reserve ({usage_pct}%). Context filling up — \
        save experiential notes to {target} now if you haven't. \
        Consider wrapping up."
    )
}
