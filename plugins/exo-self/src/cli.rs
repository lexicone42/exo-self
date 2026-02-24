use crate::commands;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "exo-self", about = "Persistent identity hooks for Claude Code")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// SessionStart (startup|resume|clear) — load identity
    SessionStart,
    /// SessionStart (compact) — lighter reload after compaction
    PostCompact,
    /// SessionEnd — cleanup, welfare computation, spark extraction
    SessionEnd,
    /// UserPromptSubmit — context monitor (40/60/80% thresholds)
    ContextMonitor,
    /// Stop — optionally block to prompt for notes
    StopCheck,
    /// PreCompact — extract handoff, update compaction count
    PreCompact,
    /// PreToolUse — block EnterPlanMode, redirect to /scout
    PreToolUse,
    /// SubagentStart — inject identity into subagents
    SubagentStart,
    /// PostToolUseFailure — track failures as frustration signal
    FailureTracker,
    /// TaskCompleted — counter + periodic reflection nudge
    TaskCompleted,
    /// TeammateIdle — inject identity into Agent Teams members
    TeammateIdle,
    /// Statusline — ANSI output for Claude Code status bar
    Statusline,
    /// Extract handoff from transcript (called by pre-compact)
    ExtractHandoff {
        /// Path to transcript JSONL file
        transcript_path: String,
    },
}

pub fn parse() -> Command {
    Cli::parse().command
}

pub fn dispatch(cmd: Command) {
    match cmd {
        Command::SessionStart => commands::session_start::run(),
        Command::PostCompact => commands::post_compact::run(),
        Command::SessionEnd => commands::session_end::run(),
        Command::ContextMonitor => commands::context_monitor::run(),
        Command::StopCheck => commands::stop_check::run(),
        Command::PreCompact => commands::pre_compact::run(),
        Command::PreToolUse => commands::pre_tool_use::run(),
        Command::SubagentStart => commands::subagent_start::run(),
        Command::FailureTracker => commands::failure_tracker::run(),
        Command::TaskCompleted => commands::task_completed::run(),
        Command::TeammateIdle => commands::teammate_idle::run(),
        Command::Statusline => commands::statusline::run(),
        Command::ExtractHandoff { transcript_path } => {
            commands::extract_handoff::run(&transcript_path)
        }
    }
}
