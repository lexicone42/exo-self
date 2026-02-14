use std::path::PathBuf;

/// All paths under ~/.claude/exo-self/
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
}

impl ExoPaths {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let root = PathBuf::from(&home).join(".claude/exo-self");
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
            root,
        }
    }

    pub fn state_file(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("state-{session_id}.json"))
    }

    pub fn handoff_file(&self, session_id: &str) -> PathBuf {
        self.handoffs_dir.join(format!("{session_id}.md"))
    }

    pub fn project_notes_dir(&self, slug: &str) -> PathBuf {
        self.per_project_dir.join(slug)
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
    }
}
