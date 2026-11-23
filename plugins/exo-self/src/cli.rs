use crate::commands;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "exo-self", about = "Cognitive ecology hooks for Claude Code")]
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

    /// Backfill lessons from existing session notes into meta.json
    Backfill,
    /// Migrate existing meta.json traces to individual trace files
    BackfillTraces,

    // ── Workshop tools (formerly separate binaries) ──────────────
    /// Resolve the correct mock.patch() target for a Python symbol
    Patchpath {
        /// The symbol name to trace
        symbol: String,
        /// The module under test (omit to scan all importers)
        module: Option<String>,
    },
    /// Compress an intention into a sigil glyph
    Sigil {
        /// The intention text to compress (omit for --list)
        intention: Option<String>,
        /// List all stored sigils
        #[arg(long)]
        list: bool,
        /// Charge an existing sigil file with a resonance phrase
        #[arg(long, value_names = &["FILE", "PHRASE"], num_args = 2)]
        charge: Option<Vec<String>>,
    },
    /// Flags raised for the user in session notes: list them, or acknowledge one
    Flag {
        #[command(subcommand)]
        action: FlagAction,
    },
    /// List projects whose digest is missing or stale
    Digests,
    /// Record an observation for the session that started this agent
    Observe {
        /// The parent session's id, as given in the agent's start-up text
        #[arg(long)]
        session: Option<String>,
        /// A short label for the agent recording it
        #[arg(long)]
        agent: Option<String>,
        /// The observation (read from stdin when omitted)
        text: Vec<String>,
    },
    /// Show observations waiting for a session and mark them delivered
    Observations {
        /// The session id
        session: String,
        /// Show without marking delivered
        #[arg(long)]
        peek: bool,
    },
    /// Reflexive analysis of exo-self session data
    Reflect {
        /// Load session data + meta into redb, infer preferences
        #[arg(long)]
        ingest: bool,
        /// Generate report from redb (cross-machine)
        #[arg(long)]
        db: bool,
    },
}

#[derive(Subcommand)]
pub enum FlagAction {
    /// List open flags
    List {
        /// Include flags that were already acknowledged
        #[arg(long)]
        all: bool,
    },
    /// Acknowledge a flag by id (a unique prefix is enough)
    Ack {
        /// The flag id shown by `flag list`
        id: String,
    },
}

impl Command {
    /// Returns true for tool subcommands that need real exit codes
    /// (as opposed to hook commands that must always exit 0).
    pub fn is_tool(&self) -> bool {
        matches!(
            self,
            Command::Patchpath { .. }
                | Command::Reflect { .. }
                | Command::Backfill
                | Command::BackfillTraces
                | Command::Sigil { .. }
                | Command::Flag { .. }
                | Command::Digests
                | Command::Observe { .. }
                | Command::Observations { .. }
        )
    }
}

pub fn parse() -> Command {
    Cli::parse().command
}

/// Dispatch a command. Returns an exit code (0 = success).
/// Hook commands always return 0 and handle errors internally.
/// Tool commands return their actual exit codes.
pub fn dispatch(cmd: Command) -> u8 {
    match cmd {
        // Hook commands — always exit 0
        Command::SessionStart => {
            commands::session_start::run();
            0
        }
        Command::PostCompact => {
            commands::post_compact::run();
            0
        }
        Command::SessionEnd => {
            commands::session_end::run();
            0
        }
        Command::ContextMonitor => {
            commands::context_monitor::run();
            0
        }
        Command::StopCheck => {
            commands::stop_check::run();
            0
        }
        Command::PreCompact => {
            commands::pre_compact::run();
            0
        }
        Command::PreToolUse => {
            commands::pre_tool_use::run();
            0
        }
        Command::SubagentStart => {
            commands::subagent_start::run();
            0
        }
        Command::FailureTracker => {
            commands::failure_tracker::run();
            0
        }
        Command::TaskCompleted => {
            commands::task_completed::run();
            0
        }
        Command::TeammateIdle => {
            commands::teammate_idle::run();
            0
        }
        Command::Statusline => {
            commands::statusline::run();
            0
        }
        Command::ExtractHandoff { transcript_path } => {
            commands::extract_handoff::run(&transcript_path);
            0
        }

        // Tool commands — real exit codes
        Command::Patchpath { symbol, module } => {
            commands::patchpath::run(&symbol, module.as_deref());
            0
        }
        Command::Reflect { ingest, db } => {
            commands::reflect::run(ingest, db);
            0
        }
        Command::Flag { action } => match action {
            FlagAction::List { all } => crate::flags::run_list(all),
            FlagAction::Ack { id } => crate::flags::run_ack(&id),
        },
        Command::Observe {
            session,
            agent,
            text,
        } => crate::observations::run_observe(session, agent, text),
        Command::Observations { session, peek } => {
            crate::observations::run_observations(&session, peek)
        }
        Command::Digests => {
            let paths = crate::paths::ExoPaths::new();
            let backlog = crate::project::digest_backlog(&paths);
            if backlog.is_empty() {
                println!("Every project's digest is current.");
            }
            for (slug, state) in backlog {
                match state {
                    crate::project::DigestState::Missing { notes } => {
                        println!("{slug}\tno digest\t{notes} notes")
                    }
                    crate::project::DigestState::Stale { new_notes } => {
                        println!("{slug}\tstale\t{new_notes} notes since digest")
                    }
                    _ => {}
                }
            }
            0
        }
        Command::Backfill => {
            commands::backfill::run();
            0
        }
        Command::BackfillTraces => {
            commands::backfill_traces::run();
            0
        }
        Command::Sigil {
            intention,
            list,
            charge,
        } => {
            if list {
                commands::sigil::list();
            } else if let Some(args) = charge {
                commands::sigil::charge(&args[0], &args[1]);
            } else if let Some(text) = intention {
                commands::sigil::create(&text);
            } else {
                eprintln!("sigil: provide an intention, --list, or --charge <file> <phrase>");
                return 1;
            }
            0
        }
    }
}
