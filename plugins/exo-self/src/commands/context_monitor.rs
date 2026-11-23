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
        let now = state::now();
        let elapsed_min = ((now - state.session_start) / 60.0) as u32;
        let min_since_note =
            project::newest_note_mtime(&paths, &slug).map(|mtime| (now - mtime) / 60.0);
        match time_checkpoint(elapsed_min, state.time_nudge_count, min_since_note, &cfg) {
            TimeCheckpoint::NotDue => {}
            TimeCheckpoint::Skip(index) => state.time_nudge_count = index,
            TimeCheckpoint::Fire(index) => {
                state.time_nudge_count = index;
                output_msg = Some(time_nudge_msg(elapsed_min, &slug));
            }
        }
    }

    // Digest suggestion: once per session, attached to whichever reflective pause
    // comes first (#24). Checked only when a pause message is going out.
    if let Some(msg) = output_msg.as_mut()
        && !state.digest_hint_fired
        && let Some(hint) = project::digest_hint(project::digest_state(&paths, &slug))
    {
        state.digest_hint_fired = true;
        msg.push_str("\n\n");
        msg.push_str(&hint);
    }

    // Effort-signal spike: capture reasoning effort if the host supplies it on this
    // (UserPromptSubmit) event. Graceful — empty input.effort leaves state untouched.
    if !input.effort.is_empty() {
        state.effort = input.effort.clone();
    }

    // Serving-model change notice. A model's self-report comes from its system prompt,
    // which can be stale: on 2026-07-27 a system prompt claimed Fable 5 for over an hour
    // after the transcript showed claude-opus-5 serving, and four session notes were
    // written with false `model:` provenance as a result. The statusline receives the
    // authoritative model from Claude Code, so surface it — but only when it CHANGES,
    // to stay quiet in the common case.
    if let Some(serving) = context_window::serving_model(&paths, session_id)
        && state.serving_model != serving
    {
        let previous = std::mem::replace(&mut state.serving_model, serving.clone());
        let notice = if previous.is_empty() {
            format!(
                "Serving model (authoritative, from Claude Code): **{serving}**. \
                 Use this for the `model:` frontmatter in session notes — your own sense \
                 of which model you are can be stale."
            )
        } else {
            format!(
                "**Serving model changed**: {previous} → **{serving}** (authoritative, from \
                 Claude Code). Mid-session model switches are silent; use the new value for \
                 `model:` frontmatter from here on."
            )
        };
        output_msg = Some(match output_msg {
            Some(existing) => format!("{notice}\n\n{existing}"),
            None => notice,
        });
    }

    state.save(&paths);

    if let Some(msg) = output_msg {
        hook_io::hook_output("UserPromptSubmit", &msg);
    } else {
        hook_io::empty_output();
    }
}

/// What to do about the time-based checkpoint on this prompt.
#[derive(Debug, PartialEq, Eq)]
enum TimeCheckpoint {
    /// No new checkpoint boundary has passed.
    NotDue,
    /// A boundary passed, but a note was written recently: count it as done, say nothing.
    Skip(u32),
    /// A boundary passed: emit the checkpoint.
    Fire(u32),
}

/// Decide the time-based checkpoint. Checkpoints fall at `first`, `first + interval`,
/// `first + 2 * interval`, ... minutes; `already_handled` is how many of those
/// boundaries this session has already fired or skipped.
///
/// Each boundary is handled once. An earlier version also fired whenever the
/// context-percentage nudge had not fired yet, which on a large window is every
/// prompt after the first hour.
///
/// A boundary is skipped when a session note for the project was written within
/// the cooldown (#26): the session has just reflected, and a checkpoint with
/// nothing new to say gets a thin entry or none.
fn time_checkpoint(
    elapsed_min: u32,
    already_handled: u32,
    min_since_note: Option<f64>,
    cfg: &Config,
) -> TimeCheckpoint {
    if elapsed_min < cfg.time_nudge_first_min {
        return TimeCheckpoint::NotDue;
    }
    // With repeats off (interval 0) only the first boundary exists.
    let boundaries_passed = (elapsed_min - cfg.time_nudge_first_min)
        .checked_div(cfg.time_nudge_interval_min)
        .map_or(1, |n| n + 1);
    if boundaries_passed <= already_handled {
        return TimeCheckpoint::NotDue;
    }
    let in_cooldown = cfg.checkpoint_cooldown_min > 0
        && min_since_note.is_some_and(|m| m >= 0.0 && m < f64::from(cfg.checkpoint_cooldown_min));
    if in_cooldown {
        TimeCheckpoint::Skip(boundaries_passed)
    } else {
        TimeCheckpoint::Fire(boundaries_passed)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn nothing_is_due_before_the_first_boundary() {
        assert_eq!(time_checkpoint(0, 0, None, &cfg()), TimeCheckpoint::NotDue);
        assert_eq!(time_checkpoint(59, 0, None, &cfg()), TimeCheckpoint::NotDue);
    }

    #[test]
    fn each_boundary_fires_once() {
        // 60 min, then every 90: boundaries at 60, 150, 240.
        assert_eq!(
            time_checkpoint(60, 0, None, &cfg()),
            TimeCheckpoint::Fire(1)
        );
        // The regression: with boundary 1 handled, later prompts in the same
        // interval say nothing, whatever the context-percentage nudge did.
        assert_eq!(time_checkpoint(61, 1, None, &cfg()), TimeCheckpoint::NotDue);
        assert_eq!(
            time_checkpoint(149, 1, None, &cfg()),
            TimeCheckpoint::NotDue
        );
        assert_eq!(
            time_checkpoint(150, 1, None, &cfg()),
            TimeCheckpoint::Fire(2)
        );
        assert_eq!(
            time_checkpoint(240, 2, None, &cfg()),
            TimeCheckpoint::Fire(3)
        );
    }

    #[test]
    fn a_long_gap_between_prompts_fires_one_checkpoint_not_several() {
        // 40 hours with no prompt: boundary 26 is current; the missed ones are not replayed.
        assert_eq!(
            time_checkpoint(2400, 1, None, &cfg()),
            TimeCheckpoint::Fire(27)
        );
        assert_eq!(
            time_checkpoint(2401, 27, None, &cfg()),
            TimeCheckpoint::NotDue
        );
    }

    #[test]
    fn a_note_written_minutes_ago_skips_the_boundary_and_consumes_it() {
        assert_eq!(
            time_checkpoint(150, 1, Some(5.0), &cfg()),
            TimeCheckpoint::Skip(2)
        );
        // Consumed: the same boundary does not fire once the cooldown lapses.
        assert_eq!(
            time_checkpoint(190, 2, Some(45.0), &cfg()),
            TimeCheckpoint::NotDue
        );
        // The next boundary fires normally.
        assert_eq!(
            time_checkpoint(240, 2, Some(95.0), &cfg()),
            TimeCheckpoint::Fire(3)
        );
    }

    #[test]
    fn an_old_note_does_not_suppress_the_checkpoint() {
        // Negative control for the cooldown: just outside it, and far outside it.
        assert_eq!(
            time_checkpoint(150, 1, Some(30.0), &cfg()),
            TimeCheckpoint::Fire(2)
        );
        assert_eq!(
            time_checkpoint(150, 1, Some(600.0), &cfg()),
            TimeCheckpoint::Fire(2)
        );
        // A note dated in the future (clock skew) is not "recent".
        assert_eq!(
            time_checkpoint(150, 1, Some(-3.0), &cfg()),
            TimeCheckpoint::Fire(2)
        );
    }

    #[test]
    fn the_knobs_are_respected() {
        let quiet = Config {
            time_nudge_first_min: 120,
            time_nudge_interval_min: 240,
            ..Config::default()
        };
        assert_eq!(
            time_checkpoint(119, 0, None, &quiet),
            TimeCheckpoint::NotDue
        );
        assert_eq!(
            time_checkpoint(120, 0, None, &quiet),
            TimeCheckpoint::Fire(1)
        );
        assert_eq!(
            time_checkpoint(359, 1, None, &quiet),
            TimeCheckpoint::NotDue
        );
        assert_eq!(
            time_checkpoint(360, 1, None, &quiet),
            TimeCheckpoint::Fire(2)
        );

        let no_cooldown = Config {
            checkpoint_cooldown_min: 0,
            ..Config::default()
        };
        assert_eq!(
            time_checkpoint(150, 1, Some(1.0), &no_cooldown),
            TimeCheckpoint::Fire(2)
        );

        let no_repeats = Config {
            time_nudge_interval_min: 0,
            ..Config::default()
        };
        assert_eq!(
            time_checkpoint(60, 0, None, &no_repeats),
            TimeCheckpoint::Fire(1)
        );
        assert_eq!(
            time_checkpoint(6000, 1, None, &no_repeats),
            TimeCheckpoint::NotDue
        );
    }
}
