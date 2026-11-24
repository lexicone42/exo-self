use std::path::PathBuf;

/// Names the notes directory when it is not the default. The hook handlers set
/// it for a host that keeps its own notes (Codex, under `~/.codex/exo-self`).
pub const HOME_VAR: &str = "EXO_SELF_HOME";

const DEFAULT_LABEL: &str = "~/.claude/exo-self";
const CLI: &str = "~/.claude/bin/exo-self";

/// The notes directory: the override when it is an absolute path, else
/// `<home>/.claude/exo-self`. A relative or empty override is ignored, so a
/// stray value cannot scatter notes around whatever directory a hook ran in.
fn root_from(home: &str, override_dir: Option<&str>) -> PathBuf {
    match override_dir.map(str::trim) {
        Some(dir) if std::path::Path::new(dir).is_absolute() => PathBuf::from(dir),
        _ => PathBuf::from(home).join(".claude/exo-self"),
    }
}

fn label_from(home: &str, override_dir: Option<&str>) -> String {
    let root = root_from(home, override_dir);
    if root == root_from(home, None) {
        DEFAULT_LABEL.to_string()
    } else {
        root.to_string_lossy().into_owned()
    }
}

fn cli_from(home: &str, override_dir: Option<&str>) -> String {
    let root = root_from(home, override_dir);
    if root == root_from(home, None) {
        CLI.to_string()
    } else {
        // The command is run by a model in its own shell, which does not have
        // the variable the hook handler set.
        format!("{HOME_VAR}='{}' {CLI}", root.to_string_lossy())
    }
}

fn env_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
}

/// The notes directory for this process.
pub fn root() -> PathBuf {
    root_from(&env_home(), std::env::var(HOME_VAR).ok().as_deref())
}

/// The notes directory as text for a message: `~/.claude/exo-self` by default,
/// the full path when another directory is in use.
pub fn root_label() -> String {
    label_from(&env_home(), std::env::var(HOME_VAR).ok().as_deref())
}

/// The command a session is told to run. It carries the directory when that is
/// not the default, so the command reaches the same notes the hooks use.
pub fn cli() -> String {
    cli_from(&env_home(), std::env::var(HOME_VAR).ok().as_deref())
}

/// All paths under the notes directory (`~/.claude/exo-self/` by default).
pub struct ExoPaths {
    pub root: PathBuf,
    pub journal: PathBuf,
    pub interests: PathBuf,
    pub config: PathBuf,
    pub meta: PathBuf,
    pub sessions_dir: PathBuf,
    pub handoffs_dir: PathBuf,
    pub per_project_dir: PathBuf,
    pub shared_state: PathBuf,
    pub context_window: PathBuf,
    pub synthesis: PathBuf,
    pub sigils_dir: PathBuf,
    pub traces_dir: PathBuf,
}

impl ExoPaths {
    pub fn new() -> Self {
        Self::at(root())
    }

    /// All paths anchored at `root`. `new()` uses `~/.claude/exo-self`; tests pass a tempdir.
    pub fn at(root: PathBuf) -> Self {
        Self {
            journal: root.join("journal.md"),
            interests: root.join("interests.md"),
            config: root.join("config.json"),
            meta: root.join("meta.json"),
            sessions_dir: root.join("sessions"),
            handoffs_dir: root.join("handoffs"),
            per_project_dir: root.join("per-project"),
            shared_state: root.join(".context-monitor-state.json"),
            context_window: root.join(".context-window.json"),
            synthesis: root.join("synthesis.md"),
            sigils_dir: root.join("sigils"),
            traces_dir: root.join("traces"),
            root,
        }
    }

    pub fn state_file(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("state-{session_id}.json"))
    }

    /// Per-session context-window file. The path itself is the session validation:
    /// reading your own file cannot return a concurrent session's usage (issue #19 —
    /// the shared .context-window.json let whichever session wrote last win).
    pub fn context_window_file(&self, session_id: &str) -> PathBuf {
        self.sessions_dir
            .join(format!("context-window-{session_id}.json"))
    }

    pub fn handoff_file(&self, session_id: &str) -> PathBuf {
        self.handoffs_dir.join(format!("{session_id}.md"))
    }

    pub fn project_notes_dir(&self, slug: &str) -> PathBuf {
        self.per_project_dir.join(slug)
    }

    /// Per-project scout report file (one-shot, consumed on session start)
    pub fn scout_file(&self, slug: &str) -> PathBuf {
        self.per_project_dir.join(slug).join("scout.md")
    }

    /// Auto-memory directory path for Claude Code's native memory system.
    /// Pass the hook input's `cwd` when available; falls back to process CWD.
    pub fn auto_memory_dir_for(&self, cwd: &str) -> Option<PathBuf> {
        let dir_path = if !cwd.is_empty() {
            PathBuf::from(cwd)
        } else {
            std::env::current_dir().ok()?
        };
        let slug = dir_path.to_string_lossy().replace(['/', '_'], "-");
        let home = std::env::var("HOME").ok()?;
        let dir = PathBuf::from(home)
            .join(".claude/projects")
            .join(slug)
            .join("memory");
        Some(dir)
    }

    /// Ensure core directories exist
    pub fn ensure_dirs(&self) {
        let _ = std::fs::create_dir_all(&self.sessions_dir);
        let _ = std::fs::create_dir_all(&self.handoffs_dir);
        let _ = std::fs::create_dir_all(&self.per_project_dir);
        let _ = std::fs::create_dir_all(self.root.join("reflections"));
        let _ = std::fs::create_dir_all(&self.sigils_dir);
        let _ = std::fs::create_dir_all(&self.traces_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_directory_and_command_are_unchanged() {
        assert_eq!(root_from("/h", None), PathBuf::from("/h/.claude/exo-self"));
        assert_eq!(label_from("/h", None), "~/.claude/exo-self");
        assert_eq!(cli_from("/h", None), "~/.claude/bin/exo-self");
    }

    #[test]
    fn an_absolute_override_moves_the_notes_and_travels_with_the_command() {
        let dir = Some("/h/.codex/exo-self");
        assert_eq!(root_from("/h", dir), PathBuf::from("/h/.codex/exo-self"));
        assert_eq!(label_from("/h", dir), "/h/.codex/exo-self");
        assert_eq!(
            cli_from("/h", dir),
            "EXO_SELF_HOME='/h/.codex/exo-self' ~/.claude/bin/exo-self"
        );
    }

    #[test]
    fn an_empty_or_relative_override_is_ignored() {
        for bad in ["", "  ", "notes", "./x"] {
            assert_eq!(root_from("/h", Some(bad)), root_from("/h", None), "{bad:?}");
            assert_eq!(cli_from("/h", Some(bad)), "~/.claude/bin/exo-self");
        }
        // Naming the default explicitly changes nothing either.
        assert_eq!(
            label_from("/h", Some("/h/.claude/exo-self")),
            "~/.claude/exo-self"
        );
    }
}
