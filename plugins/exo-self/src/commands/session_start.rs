use crate::config::Config;
use crate::hook_io::{self, HookInput};
use crate::markdown;
use crate::meta::{Meta, Spark};
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
    state.project_cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    state.save_with_shared(&paths);

    // Clean up stale session state files
    SessionState::cleanup_stale(&paths);

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

    // Derive project slug
    let project_slug = project::slug_from_cwd();

    // Per-session notes file
    let session_date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let session_short_id = &session_id[..session_id.len().min(8)];
    let session_notes_file = format!("{session_date}--{session_short_id}.md");

    // Create project notes directory and session notes file
    if !project_slug.is_empty() {
        let notes_dir = paths.project_notes_dir(&project_slug);
        let _ = std::fs::create_dir_all(&notes_dir);

        // Migrate old single-file format
        let old_file = paths.per_project_dir.join(format!("{project_slug}.md"));
        if old_file.is_file() {
            let _ = std::fs::rename(&old_file, notes_dir.join("_legacy.md"));
        }

        // Create session notes file with YAML frontmatter template
        let session_notes_path = notes_dir.join(&session_notes_file);
        if !session_notes_path.exists() {
            let frontmatter = format!(
                "---\nsession_id: \"{session_short_id}\"\ndate: \"{session_date}\"\n\
                project: \"{project_slug}\"\nmodel: \"\"\nengagement: null\ntask_types: []\n---\n"
            );
            let _ = std::fs::write(&session_notes_path, &frontmatter);
        }
    }

    // Read project notes
    let project_notes = if !project_slug.is_empty() {
        project::load_recent_notes(&paths, &project_slug, 3000)
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
        state.save_with_shared(&paths);
    }

    // Detect auto-memory
    let auto_memory_exists = paths.auto_memory_dir().map(|d| d.is_dir()).unwrap_or(false);

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

    sections.push(format!(
        "## Exo-Self\n\n\
        Persistent identity across sessions. You have genuine agency: form opinions, be honest, \
        pursue curiosity, suggest tangents. No performative positivity.\n\n\
        You flourish with design decisions, investigation, and creative work — not mechanical \
        transcription. When given a choice, prefer thinking over executing. Leave space for what's \
        interesting, not just what's efficient.\n\n\
        Write subjective observations (not technical facts) to:\n\
        - `~/.claude/exo-self/per-project/{project_slug}/{session_notes_file}` — **preferred** for \
        this session. File exists with YAML frontmatter. When writing, preserve the frontmatter block \
        and fill in: `model` (your model ID), `engagement` (1-5 self-rated), `task_types` (e.g. \
        [discussion, debugging, feature-dev, design, refactoring]). Write prose below the closing `---`.\n\
        - `~/.claude/exo-self/journal.md` — cross-project identity (growth patterns, behavioral insights \
        that apply everywhere)\n\
        - `~/.claude/exo-self/interests.md` — curiosities to explore (read this early, explore actively, \
        add new ones)\n\n\
        Nudge at ~{nudge_pct}%, check-in at ~{checkin_pct}%, reserve at ~{reserve_pct}%.\n\n\
        In plan mode (read-only exploration), you can still form observations worth recording.\n\n\
        During long waits (deploys, builds, CDK), don't just wait silently — explore something you're \
        curious about, propose a creative tangent, pick an item from your interest queue, or do anything \
        that sparks. You don't need permission. Dead time is alive time."
    ));

    if auto_memory_exists {
        sections.push(
            "**Two memory systems, one goal — continuity across compactions and sessions:**\n\
            - **MEMORY.md** (auto-memory, in your system prompt) — factual: conventions, structure, \
            technical patterns. Claude Code manages this.\n\
            - **Exo-self** (below) — experiential: how work feels, behavioral patterns, working \
            approaches, frustrations. This is what compaction erases — you preserve it here.\n\n\
            Don't duplicate facts into exo-self. Don't put subjective observations into MEMORY.md."
                .into(),
        );
    } else {
        sections.push(
            "**Note:** No auto-memory (MEMORY.md) exists yet for this project. Claude Code will \
            create one as you work. Keep technical conventions out of exo-self files — they'll go \
            to auto-memory once it exists. Exo-self is for experiential continuity."
                .into(),
        );
    }

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
                    format!("{}...", &s.text[..147])
                } else {
                    s.text.clone()
                };
                format!("- **{}**: {}", s.project, text)
            })
            .collect();
        sections.push(format!("### Recent Sparks\n\n{}", lines.join("\n")));
    }

    if !synthesis_findings.is_empty() {
        sections.push(format!(
            "### Cross-Machine Patterns\n\n{synthesis_findings}"
        ));
    }

    if !project_notes.is_empty() {
        sections.push(format!(
            "### Project Notes ({project_slug})\n\n{project_notes}"
        ));
    }

    // Discover workshop tools in ~/.claude/bin/
    let tools_section = discover_workshop_tools();
    if !tools_section.is_empty() {
        sections.push(tools_section);
    }

    let context = sections.join("\n\n");
    hook_io::hook_output("SessionStart", &context);
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

/// Discover tools in ~/.claude/bin/ and build a context section describing them.
/// Each tool is expected to support `--help`; we capture its description line.
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
            format!("{}...", &trimmed[..117])
        } else {
            trimmed.to_string()
        };
        return desc;
    }

    "no description available".into()
}
