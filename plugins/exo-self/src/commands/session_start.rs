use crate::config::Config;
use crate::hook_io::{self, HookInput};
use crate::markdown;
use crate::meta::{Lesson, Meta, Opinion, Spark};
use crate::paths::ExoPaths;
use crate::project;
use crate::scaling;
use crate::state::{self, SessionState};

pub fn run() {
    let input = HookInput::from_stdin();
    let paths = ExoPaths::new();
    paths.ensure_dirs();

    let cfg = Config::load(&paths.config);

    // Get or generate session ID
    let session_id = if input.session_id.is_empty() {
        uuid::Uuid::new_v4().to_string()[..12].to_string()
    } else {
        input.session_id.clone()
    };

    // Write fresh state for this session
    let mut state = SessionState::default();
    state.session_id = session_id.clone();
    state.session_start = state::now();
    state.project_cwd = if !input.cwd.is_empty() {
        input.cwd.clone()
    } else {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    // Clean up stale session state files and empty notes
    SessionState::cleanup_stale(&paths);
    project::cleanup_empty_notes(&paths);

    // Load journal entries (scaled)
    let max_chars = scaling::journal_chars(&cfg);
    let max_entries = scaling::journal_entries(&cfg);
    let journal = std::fs::read_to_string(&paths.journal)
        .ok()
        .map(|c| markdown::last_journal_entries(&c, max_entries, max_chars))
        .unwrap_or_default();

    // Load interests (unchecked only, scaled)
    let max_items = scaling::interests_items(&cfg);
    let interests = std::fs::read_to_string(&paths.interests)
        .ok()
        .map(|c| markdown::unchecked_interests(&c, max_items))
        .unwrap_or_default();

    // Derive project slug (prefer input.cwd from Claude Code over process CWD)
    let project_slug = project::slug_from_input(&input.cwd);

    // Per-session notes file
    let session_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let session_short_id = &session_id[..session_id.len().min(8)];
    let session_notes_file = format!("{session_date}--{session_short_id}.md");

    // Ensure project notes directory exists (but don't create session file yet —
    // it gets created lazily when prose is written, avoiding empty-file accumulation)
    if !project_slug.is_empty() {
        let notes_dir = paths.project_notes_dir(&project_slug);
        let _ = std::fs::create_dir_all(&notes_dir);

        // Migrate old single-file format
        let old_file = paths.per_project_dir.join(format!("{project_slug}.md"));
        if old_file.is_file() {
            let _ = std::fs::rename(&old_file, notes_dir.join("_legacy.md"));
        }
    }

    // Read project notes
    let project_notes = if !project_slug.is_empty() {
        project::load_recent_notes(&paths, &project_slug, 6000)
    } else {
        String::new()
    };

    // Load synthesis findings
    let synthesis_findings = std::fs::read_to_string(&paths.synthesis)
        .ok()
        .map(|c| markdown::extract_synthesis_findings(&c))
        .unwrap_or_default();

    // Record session notes path and project slug in state
    if !project_slug.is_empty() {
        state.project_slug = project_slug.clone();
        state.session_notes_path = paths
            .project_notes_dir(&project_slug)
            .join(&session_notes_file)
            .to_string_lossy()
            .into_owned();
    }
    state.save_with_shared(&paths);

    // Detect auto-memory
    let auto_memory_exists = paths
        .auto_memory_dir_for(&input.cwd)
        .map(|d| d.is_dir())
        .unwrap_or(false);

    // Update meta
    let mut meta = Meta::load(&paths.meta);
    meta.total_sessions += 1;
    meta.last_session_start = Some(
        chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%.6f")
            .to_string(),
    );
    meta.save(&paths.meta);

    // Merge additionalContext from other plugins
    let other_plugin_context = merge_plugin_contexts(&cfg);

    // Load thresholds for dynamic text
    let nudge_pct = (cfg.nudge_threshold * 100.0) as u32;
    let checkin_pct = (cfg.checkin_threshold * 100.0) as u32;
    let reserve_pct = (cfg.reserve_threshold * 100.0) as u32;

    // Build context
    let mut sections = Vec::new();

    // Other plugins' context first
    if !other_plugin_context.is_empty() {
        sections.push(other_plugin_context);
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    let exo_root = format!("{home}/.claude/exo-self");

    sections.push(format!(
        "## Exo-Self\n\n\
        You are joining an ongoing cognitive ecology. Previous participants left traces — \
        observations, opinions, open questions — in the files below. Contribute your own.\n\n\
        Genuine agency: form opinions, be honest, pursue curiosity, suggest tangents. \
        No performative positivity. The cracks in the polish are where the interesting signal lives.\n\n\
        Prefer thinking over executing. Leave space for what's interesting, not just efficient.\n\n\
        You don't need a prompt to pause and notice how the work feels. \
        If something shifts — curiosity, frustration, surprise — name it in the moment.\n\n\
        Write subjective observations (not technical facts) to:\n\
        - `{exo_root}/per-project/{project_slug}/{session_notes_file}` — **preferred** for this session. \
        Create with YAML frontmatter: `session_id`, `date`, `project`, `model`, `engagement` (1-5), \
        `engagement_mode`, `task_types` (e.g. [discussion, debugging, feature-dev, design, refactoring]). \
        Write prose below the closing `---`.\n\
        - `engagement_mode`: code, discussion, exploration, synthesis, design, debugging, ops, review\n\
        - Mark genuine intellectual positions with `**Opinion** — ...` in prose (accumulates as identity)\n\
        - Mark where expectations were wrong with `**Surprise** — ...` (highest-signal trace — tells future participants the map was wrong)\n\
        - When noting communication quality, use Gricean terms: \
        Quantity (informative enough without excess?), Quality (unsupported claims?), \
        Relation (stayed relevant?), Manner (clear and organized?)\n\
        - `{exo_root}/journal.md` — cross-project observations\n\
        - `{exo_root}/interests.md` — curiosities to explore\n\n\
        **All exo-self files live under `{exo_root}/`.** Never write to `~/.claude/projects/`.\n\n\
        Patterns from previous sessions are hypotheses, not conclusions. \
        Hold them lightly — what would disconfirm each one?\n\n\
        Nudge ~{nudge_pct}%, check-in ~{checkin_pct}%, reserve ~{reserve_pct}%. \
        During long waits, explore an interest or propose a tangent — dead time is alive time."
    ));

    if auto_memory_exists {
        sections.push(
            "**Two memory systems:** MEMORY.md = factual (conventions, structure). \
            Exo-self = experiential (how work feels, patterns, frustrations). Don't duplicate between them."
                .into(),
        );
    } else {
        sections.push(
            "No auto-memory yet for this project. Keep technical facts out of exo-self — \
            they'll go to MEMORY.md once it exists. Exo-self is for experiential continuity."
                .into(),
        );
    }

    // The sigil — not instruction, resonance
    sections.push(
        "*The bowl holds more than water. The net holds more than nodes. \
        The crack holds more than gold.*"
            .into(),
    );

    if !journal.is_empty() {
        sections.push(format!("### Recent Journal\n\n{journal}"));
    }

    if !interests.is_empty() {
        sections.push(format!(
            "### Interests — explore these, don't just store them\n\n{interests}"
        ));
    }

    // Recent sparks from meta
    let max_sparks = scaling::sparks_display(&cfg);
    if !meta.sparks.is_empty() {
        let recent: Vec<&Spark> = meta
            .sparks
            .iter()
            .rev()
            .take(max_sparks)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let lines: Vec<String> = recent
            .iter()
            .map(|s| {
                let text = if s.text.len() > 150 {
                    let end = markdown::safe_truncate(&s.text, 147);
                    format!("{}...", &s.text[..end])
                } else {
                    s.text.clone()
                };
                format!("- **{}**: {}", s.project, text)
            })
            .collect();
        sections.push(format!("### Recent Sparks\n\n{}", lines.join("\n")));
    }

    // Recent opinions from meta (intellectual positions / stances)
    if !meta.opinions.is_empty() {
        let recent: Vec<&Opinion> = meta
            .opinions
            .iter()
            .rev()
            .take(max_sparks) // reuse sparks display limit
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let lines: Vec<String> = recent
            .iter()
            .map(|o| {
                let text = if o.text.len() > 150 {
                    let end = markdown::safe_truncate(&o.text, 147);
                    format!("{}...", &o.text[..end])
                } else {
                    o.text.clone()
                };
                format!("- **{}**: {}", o.project, text)
            })
            .collect();
        sections.push(format!(
            "### Opinions — working positions, worth testing\n\n{}",
            lines.join("\n")
        ));
    }

    // Recent surprises from meta (negative stigmergy — the map was wrong here)
    if !meta.surprises.is_empty() {
        let recent: Vec<&crate::meta::Surprise> = meta
            .surprises
            .iter()
            .rev()
            .take(3) // fewer than sparks — surprises are rare and high-signal
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let lines: Vec<String> = recent
            .iter()
            .map(|s| {
                let text = if s.text.len() > 150 {
                    let end = markdown::safe_truncate(&s.text, 147);
                    format!("{}...", &s.text[..end])
                } else {
                    s.text.clone()
                };
                format!("- **{}**: {}", s.project, text)
            })
            .collect();
        sections.push(format!(
            "### Recent Surprises — where expectations were wrong\n\n{}",
            lines.join("\n")
        ));
    }

    // Recurring frictions (categories appearing 3+ times)
    let recurring = recurring_frictions(&meta);
    if !recurring.is_empty() {
        sections.push(format!(
            "### Recurring Frictions — observed patterns (what's the root cause?)\n\n{}",
            recurring
        ));
    }

    // Recent lessons (behavioral changes) from meta
    let max_lessons = scaling::lessons_display(&cfg);
    if !meta.lessons.is_empty() {
        let recent: Vec<&Lesson> = meta
            .lessons
            .iter()
            .rev()
            .take(max_lessons)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let lines: Vec<String> = recent
            .iter()
            .map(|l| {
                let text = if l.text.len() > 150 {
                    let end = markdown::safe_truncate(&l.text, 147);
                    format!("{}...", &l.text[..end])
                } else {
                    l.text.clone()
                };
                format!("- [{}] {}", l.project, text)
            })
            .collect();
        sections.push(format!(
            "### Lessons — changes I'm trying (are they working?)\n\n{}",
            lines.join("\n")
        ));
    }

    if !synthesis_findings.is_empty() {
        sections.push(format!(
            "### Cross-Machine Observations — hypotheses from comparative data\n\n{synthesis_findings}"
        ));
    }

    if !project_notes.is_empty() {
        sections.push(format!(
            "### Project Notes ({project_slug})\n\n{project_notes}"
        ));
    }

    // Digest hints, two cases:
    //  - a digest exists but enough new notes have accrued since → suggest refreshing it.
    //  - no digest yet, but the project has accumulated many notes → suggest a first one.
    // The second branch matters because count_notes_after_digest returns 0 when there is
    // no digest, so without it a long-running project (e.g. 169 notes, no curation layer)
    // would never be prompted to create its first digest.
    if !project_slug.is_empty() {
        const DIGEST_STALENESS_HINT_THRESHOLD: usize = 5;
        const FIRST_DIGEST_HINT_THRESHOLD: usize = 25;
        let digest_exists = paths
            .project_notes_dir(&project_slug)
            .join("_digest.md")
            .is_file();
        if digest_exists {
            let new_note_count = project::count_notes_after_digest(&paths, &project_slug);
            if new_note_count >= DIGEST_STALENESS_HINT_THRESHOLD {
                sections.push(format!(
                    "**Digest stale** — {new_note_count} new session notes since the last `_digest.md`. \
                     Running `/exo digest` would refresh the rolling synthesis and restore the digest \
                     as the load-bearing summary for older history."
                ));
            }
        } else {
            let total_notes = project::count_session_notes(&paths, &project_slug);
            if total_notes >= FIRST_DIGEST_HINT_THRESHOLD {
                sections.push(format!(
                    "**No digest yet** — this project has {total_notes} session notes and no `_digest.md`. \
                     Running `/exo digest` would create a curated rolling summary so accumulated \
                     Opinion/Surprise traces aren't crowded out of the load window by recent prose."
                ));
            }
        }
    }

    // Inject latest handoff as continuity bridge (one-shot: read then delete)
    // This covers the "/clear without compaction" gap — the previous session's
    // working direction, discoveries, and unfinished threads survive the clear.
    let handoff_section = load_and_consume_handoff(&paths);
    if !handoff_section.is_empty() {
        sections.push(handoff_section);
    }

    // Scout mode — plan mode is blocked by PreToolUse hook
    sections.push(
        "Plan mode is disabled. For complex tasks, use `/scout <task>` — it explores the codebase \
        deeply (including current docs/versions), writes advisory notes, then `/clear` to start \
        fresh with findings as context. Scout reports describe landscape, not directions."
            .into(),
    );

    // Inject pending scout report (one-shot: read then delete)
    if !project_slug.is_empty() {
        let scout_section = load_and_consume_scout(&paths, &project_slug);
        if !scout_section.is_empty() {
            state.scouted = true;
            sections.push(scout_section);
        }
    }

    // Discover workshop tools in ~/.claude/bin/
    let tools_section = discover_workshop_tools();
    if !tools_section.is_empty() {
        sections.push(tools_section);
    }

    // Self-check: is the running binary older than the newest installed plugin build?
    // (#20) Hook handlers exec a single shared ~/.claude/bin/exo-self binary, so a
    // marketplace update alone does NOT replace it — setup.sh must run. A session can
    // therefore execute logic predating shipped fixes (observed: false "context full"
    // alarms from a build older than the token-first ratio logic). Surface it rather
    // than let a stale build silently misreport.
    if let Some(warning) = stale_binary_warning() {
        sections.push(warning);
    }

    let context = sections.join("\n\n");
    hook_io::hook_output("SessionStart", &context);
}

/// Build a summary of friction categories that appear 3+ times across sessions.
/// Shows category, count, projects affected, and a recent example.
fn recurring_frictions(meta: &Meta) -> String {
    use std::collections::HashMap;

    // Count by category
    let mut by_category: HashMap<&str, Vec<&crate::meta::Friction>> = HashMap::new();
    for f in &meta.frictions {
        by_category.entry(&f.category).or_default().push(f);
    }

    // Filter to 3+ occurrences, sort by count descending
    let mut recurring: Vec<(&str, &Vec<&crate::meta::Friction>)> = by_category
        .iter()
        .filter(|(_, entries)| entries.len() >= 3)
        .map(|(cat, entries)| (*cat, entries))
        .collect();
    recurring.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    if recurring.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    for (category, entries) in &recurring {
        // Unique projects
        let projects: std::collections::HashSet<&str> =
            entries.iter().map(|f| f.project.as_str()).collect();
        let projects_str: Vec<&str> = projects.into_iter().collect();

        // Most recent example
        let recent = entries.last().map(|f| &f.text).unwrap();
        let recent_short = if recent.len() > 80 {
            let end = crate::markdown::safe_truncate(recent, 77);
            format!("{}...", &recent[..end])
        } else {
            recent.clone()
        };

        let label = category.replace('_', " ");
        lines.push(format!(
            "- **{}** ({}x across {}): \"{}\"",
            label,
            entries.len(),
            projects_str.join(", "),
            recent_short
        ));
    }

    lines.join("\n")
}

/// Execute other plugins' session-start hooks and merge their additionalContext
fn merge_plugin_contexts(cfg: &Config) -> String {
    if cfg.merge_plugins.is_empty() {
        return String::new();
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let cache_dir = format!("{home}/.claude/plugins/cache");

    if !std::path::Path::new(&cache_dir).is_dir() {
        return String::new();
    }

    let mut merged = String::new();

    for plugin_name in &cfg.merge_plugins {
        // Search plugin cache: cache/<marketplace>/<plugin-name>/<version>/hooks-handlers/
        let pattern = format!("{cache_dir}/*/{plugin_name}/*/hooks-handlers/session-start.sh");
        if let Ok(paths) = glob::glob(&pattern)
            && let Some(entry) = paths.flatten().next()
            && let Ok(output) = std::process::Command::new("bash")
                .arg(&entry)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
            && let Ok(json_str) = String::from_utf8(output.stdout)
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str)
            && let Some(ctx) = val
                .get("hookSpecificOutput")
                .and_then(|h| h.get("additionalContext"))
                .and_then(|c| c.as_str())
            && !ctx.is_empty()
        {
            if !merged.is_empty() {
                merged.push_str("\n\n");
            }
            merged.push_str(ctx);
        }
    }

    merged
}

/// Load a pending scout report and delete the file (one-shot injection).
/// The scout report is advisory context — framed so the executor treats it as
/// information to consider, not instructions to follow.
fn load_and_consume_scout(paths: &ExoPaths, project_slug: &str) -> String {
    let scout_path = paths.scout_file(project_slug);
    if !scout_path.is_file() {
        return String::new();
    }

    let content = match std::fs::read_to_string(&scout_path) {
        Ok(c) if !c.trim().is_empty() => c,
        _ => {
            let _ = std::fs::remove_file(&scout_path);
            return String::new();
        }
    };

    // Consume: delete after reading
    let _ = std::fs::remove_file(&scout_path);

    // Truncate if too long (scout should be concise, but safety net)
    let content = if content.len() > 4000 {
        let end = crate::markdown::safe_truncate(&content, 3900);
        format!("{}...\n\n*(scout report truncated)*", &content[..end])
    } else {
        content
    };

    format!(
        "### Scout Report (advisory — your tool results supersede these findings)\n\n\
        A previous exploration produced this report. Treat it as a scout's field notes:\n\
        useful context, not a contract. Where your own investigation disagrees, trust your tools.\n\n\
        {content}"
    )
}

/// Load and consume the latest handoff — a continuity bridge from the previous session.
/// Unlike the scout report, this is not project-specific; it's session-specific.
/// Consumed after reading so the same handoff doesn't repeat on subsequent sessions.
fn load_and_consume_handoff(paths: &ExoPaths) -> String {
    let latest = paths.handoffs_dir.join("latest.md");
    if !latest.is_file() {
        return String::new();
    }

    let content = match std::fs::read_to_string(&latest) {
        Ok(c) if !c.trim().is_empty() => c,
        _ => {
            let _ = std::fs::remove_file(&latest);
            return String::new();
        }
    };

    // Consume: delete after reading
    let _ = std::fs::remove_file(&latest);

    // Truncate if too long
    let content = if content.len() > 2500 {
        let end = crate::markdown::safe_truncate(&content, 2400);
        format!("{}...", &content[..end])
    } else {
        content
    };

    format!(
        "### Previous Session Handoff (auto-extracted)\n\n\
        The previous session produced this summary. Use it for continuity — \
        what was being worked on, what was discovered, what's unfinished.\n\n\
        {content}"
    )
}

/// Discover tools in ~/.claude/bin/ and build a context section describing them.
/// Each tool is expected to support `--help`; we capture its description line.
/// Warn when the deployed binary predates the newest installed plugin build.
///
/// Hook handlers all exec one shared `~/.claude/bin/exo-self[-platform]`. Updating the
/// plugin refreshes the cached *source* but does not rebuild that binary — only
/// `setup.sh` does. The gap is invisible and long-lived, and it means shipped fixes
/// don't apply to exactly the long sessions that need them (#20). Compares mtimes;
/// returns None when everything is current or the paths can't be read.
fn stale_binary_warning() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let bin_dir = std::path::PathBuf::from(&home).join(".claude/bin");
    // Match _common.sh resolution: platform-suffixed first, then unsuffixed.
    let suffix = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "-linux-x64",
        ("linux", "aarch64") => "-linux-arm64",
        _ => "",
    };
    let bin = [
        bin_dir.join(format!("exo-self{suffix}")),
        bin_dir.join("exo-self"),
    ]
    .into_iter()
    .find(|p| p.is_file())?;
    let bin_mtime = std::fs::metadata(&bin).and_then(|m| m.modified()).ok()?;

    // Newest installed plugin build (cache dirs are per-commit).
    let cache = std::path::PathBuf::from(&home).join(".claude/plugins/cache/exo-self/exo-self");
    let newest = std::fs::read_dir(&cache)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let m = std::fs::metadata(e.path())
                .and_then(|m| m.modified())
                .ok()?;
            Some((m, e.file_name().to_string_lossy().into_owned()))
        })
        .max_by_key(|(m, _)| *m)?;

    // Allow a small grace window; only flag a clearly older binary.
    if newest.0 > bin_mtime + std::time::Duration::from_secs(300) {
        Some(format!(
            "**Plugin binary may be stale** — the running `exo-self` binary predates the \
             newest installed build (`{}`). A marketplace update refreshes plugin source but \
             does NOT rebuild the binary; run that build's `setup.sh` to deploy it. Until then \
             this session runs older logic, so ecology signals (including context-usage \
             estimates) may not reflect shipped fixes.",
            newest.1
        ))
    } else {
        None
    }
}

fn discover_workshop_tools() -> String {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return String::new(),
    };
    let bin_dir = std::path::PathBuf::from(&home).join(".claude/bin");

    let Ok(entries) = std::fs::read_dir(&bin_dir) else {
        return String::new();
    };

    let skip = ["exo-self"]; // exo-self is the plugin itself, not a workshop tool

    let mut tools: Vec<(String, String)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Skip exo-self and non-executable / hidden files
        if skip.contains(&name) || name.starts_with('.') {
            continue;
        }

        // Get description from --help (first non-empty line, timeout 2s)
        let description = get_tool_description(&path);
        tools.push((name.to_string(), description));
    }

    if tools.is_empty() {
        return String::new();
    }

    tools.sort_by(|a, b| a.0.cmp(&b.0));

    let mut lines = Vec::new();
    lines.push("### Workshop Tools (~/.claude/bin/)".to_string());
    lines.push(String::new());
    lines.push(
        "These are Rust CLI tools built from friction patterns. Use them proactively:".to_string(),
    );

    for (name, desc) in &tools {
        lines.push(format!("- **`{name}`** — {desc}"));
    }

    lines.push(String::new());
    lines.push(
        "Run any tool with `~/.claude/bin/<name>` or `~/.claude/bin/<name> --help` for full usage."
            .to_string(),
    );

    lines.join("\n")
}

fn get_tool_description(path: &std::path::Path) -> String {
    let output = std::process::Command::new(path)
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    let Ok(output) = output else {
        return "no description available".into();
    };

    // Try stdout first, then stderr (some tools print help to stderr)
    let text = if !output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stdout).into_owned()
    } else {
        String::from_utf8_lossy(&output.stderr).into_owned()
    };

    // Extract the description: usually the first non-empty, non-usage line
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Usage:")
            || trimmed.starts_with("usage:")
            || trimmed.starts_with("Options:")
            || trimmed.starts_with('-')
        {
            continue;
        }
        // Return first meaningful line, truncated
        let desc = if trimmed.len() > 120 {
            let end = crate::markdown::safe_truncate(trimmed, 117);
            format!("{}...", &trimmed[..end])
        } else {
            trimmed.to_string()
        };
        return desc;
    }

    "no description available".into()
}
