//! preflight — run formatters + linters + stage before committing.
//!
//! Detects project type from files in the current directory (or git root)
//! and runs the appropriate fixers. Stages any modified files afterward
//! so the next `git commit` lands clean on the first try.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Project {
    kind: &'static str,
    path: PathBuf,
}

/// Returns exit code: 0 on success, 1 on failure.
pub fn run(dry_run: bool) -> u8 {
    let projects = detect_projects();

    if projects.is_empty() {
        eprintln!("preflight: no recognized project files found");
        return 1;
    }

    eprintln!("preflight: found {} project(s)", projects.len());

    let mut any_failure = false;

    for project in &projects {
        let label = if project.path == Path::new(".") {
            project.kind.to_string()
        } else {
            format!("{} ({})", project.kind, project.path.display())
        };
        eprintln!("  [{label}]");

        let ok = match project.kind {
            "python" => run_python_preflight(&project.path, dry_run),
            "rust" => run_rust_preflight(&project.path, dry_run),
            "node" => run_node_preflight(&project.path, dry_run),
            _ => true,
        };

        if !ok {
            any_failure = true;
        }
    }

    if !dry_run {
        stage_changes();
    }

    if any_failure {
        1
    } else {
        eprintln!("preflight: all checks passed");
        0
    }
}

fn detect_projects() -> Vec<Project> {
    let mut projects = Vec::new();

    // Check current directory first
    detect_at(Path::new("."), &mut projects);

    // If nothing found, scan immediate subdirectories (monorepo support)
    if projects.is_empty()
        && let Ok(entries) = std::fs::read_dir(".")
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !is_hidden(&path) {
                detect_at(&path, &mut projects);
            }
        }
    }

    // If still nothing, scan two levels deep
    if projects.is_empty()
        && let Ok(entries) = std::fs::read_dir(".")
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && !is_hidden(&path)
                && let Ok(sub_entries) = std::fs::read_dir(&path)
            {
                for sub_entry in sub_entries.flatten() {
                    let sub_path = sub_entry.path();
                    if sub_path.is_dir() && !is_hidden(&sub_path) {
                        detect_at(&sub_path, &mut projects);
                    }
                }
            }
        }
    }

    projects
}

fn detect_at(dir: &Path, projects: &mut Vec<Project>) {
    if dir.join("pyproject.toml").exists() || dir.join("ruff.toml").exists() {
        projects.push(Project {
            kind: "python",
            path: dir.to_path_buf(),
        });
    }
    if dir.join("Cargo.toml").exists() {
        projects.push(Project {
            kind: "rust",
            path: dir.to_path_buf(),
        });
    }
    if dir.join("package.json").exists() {
        projects.push(Project {
            kind: "node",
            path: dir.to_path_buf(),
        });
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
        n.starts_with('.') || n == "node_modules" || n == "target" || n == "__pycache__"
    })
}

fn run_python_preflight(dir: &Path, dry_run: bool) -> bool {
    let mut ok = true;

    if which("ruff") {
        let dir_str = dir.to_str().unwrap_or(".");
        if dry_run {
            eprintln!("    would run: ruff format {dir_str}");
            eprintln!("    would run: ruff check --fix {dir_str}");
        } else {
            eprintln!("    ruff format...");
            if !run_in(dir, &["ruff", "format", "."]) {
                ok = false;
            }
            eprintln!("    ruff check --fix...");
            if !run_in(dir, &["ruff", "check", "--fix", "."]) {
                eprintln!("    ruff check found unfixable issues (fixes still applied)");
            }
        }
    } else {
        eprintln!("    ruff not found, skipping");
    }

    ok
}

fn run_rust_preflight(dir: &Path, dry_run: bool) -> bool {
    let mut ok = true;
    let timeout = clippy_timeout();

    if which("cargo") {
        if dry_run {
            eprintln!("    would run: cargo fmt");
            eprintln!(
                "    would run: cargo clippy --fix (changed crates only, {}s timeout)",
                timeout.as_secs()
            );
            eprintln!("    would run: cargo fmt (second pass — clippy --fix can break formatting)");
        } else {
            // Pass 1: fmt to normalize formatting before clippy
            eprintln!("    cargo fmt...");
            if !run_in(dir, &["cargo", "fmt"]) {
                ok = false;
            }

            // Pass 2: clippy --fix with changed-crate filtering and timeout
            let ran_clippy = run_clippy_filtered(dir, timeout);

            // Pass 3: fmt again to clean up clippy's rewrites (only if clippy ran)
            if ran_clippy {
                eprintln!("    cargo fmt (post-clippy cleanup)...");
                if !run_in(dir, &["cargo", "fmt"]) {
                    ok = false;
                }
            }
        }
    } else {
        eprintln!("    cargo not found, skipping");
    }

    ok
}

/// Run clippy with changed-crate filtering (workspaces) and timeout.
/// Returns true if clippy actually ran (so caller knows whether to re-fmt).
fn run_clippy_filtered(dir: &Path, timeout: Duration) -> bool {
    match changed_rust_crates(dir) {
        Some(crates) if crates.is_empty() => {
            eprintln!("    clippy: no changed crates, skipping");
            false
        }
        Some(crates) => {
            let crate_list = crates.join(", ");
            eprintln!("    cargo clippy --fix -p {{{crate_list}}}...");
            let mut cmd = Command::new("cargo");
            cmd.args([
                "clippy",
                "--fix",
                "--allow-dirty",
                "--allow-staged",
                "--all-targets",
            ]);
            for name in &crates {
                cmd.args(["-p", name]);
            }
            cmd.current_dir(dir);
            report_clippy_result(run_cmd_with_timeout(&mut cmd, timeout), timeout);
            true
        }
        None => {
            // Single crate (not a workspace) — run clippy on everything with timeout
            eprintln!("    cargo clippy --fix...");
            let mut cmd = Command::new("cargo");
            cmd.args([
                "clippy",
                "--fix",
                "--allow-dirty",
                "--allow-staged",
                "--all-targets",
            ]);
            cmd.current_dir(dir);
            report_clippy_result(run_cmd_with_timeout(&mut cmd, timeout), timeout);
            true
        }
    }
}

fn report_clippy_result(result: Option<bool>, timeout: Duration) {
    match result {
        Some(true) => {}
        Some(false) => {
            eprintln!("    clippy found unfixable issues (fixes still applied)");
        }
        None => {
            eprintln!(
                "    clippy timed out after {}s, skipping (set PREFLIGHT_CLIPPY_TIMEOUT to adjust)",
                timeout.as_secs()
            );
        }
    }
}

/// Detect which crate packages have changed files in a workspace.
/// Returns `None` if this isn't a workspace (single crate — no filtering needed).
/// Returns `Some(vec![])` if it's a workspace but no crates have changes.
fn changed_rust_crates(dir: &Path) -> Option<Vec<String>> {
    let cargo_toml = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    if !cargo_toml.contains("[workspace]") {
        return None;
    }

    // Get changed files: staged + unstaged
    let staged = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let unstaged = Command::new("git")
        .args(["diff", "--name-only"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    let mut files: Vec<String> = Vec::new();
    for output in [&staged.stdout, &unstaged.stdout] {
        let text = String::from_utf8_lossy(output);
        for line in text.lines() {
            if !line.is_empty() {
                files.push(line.to_string());
            }
        }
    }
    files.sort();
    files.dedup();

    // Map changed files to crate package names
    let mut crate_names: Vec<String> = Vec::new();
    for file in &files {
        if let Some(name) = find_crate_for_file(dir, Path::new(file))
            && !crate_names.contains(&name)
        {
            crate_names.push(name);
        }
    }

    Some(crate_names)
}

/// Walk up from a file path to find its owning crate's package name.
fn find_crate_for_file(workspace_root: &Path, file: &Path) -> Option<String> {
    let full = workspace_root.join(file);
    let mut search_dir = full.parent()?.to_path_buf();

    loop {
        let cargo_toml = search_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            return extract_package_name(&cargo_toml);
        }
        // Don't go above workspace root
        if search_dir
            == workspace_root
                .canonicalize()
                .unwrap_or_else(|_| workspace_root.into())
            || !search_dir.starts_with(workspace_root)
        {
            break;
        }
        if !search_dir.pop() {
            break;
        }
    }
    None
}

/// Extract `name = "..."` from the [package] section of a Cargo.toml
/// without pulling in a TOML parser.
fn extract_package_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') {
            if in_package {
                break;
            }
            continue;
        }
        if in_package
            && let Some((key, value)) = trimmed.split_once('=')
            && key.trim() == "name"
        {
            return Some(
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            );
        }
    }
    None
}

/// Get clippy timeout from PREFLIGHT_CLIPPY_TIMEOUT env var (seconds), default 30s.
fn clippy_timeout() -> Duration {
    let secs = std::env::var("PREFLIGHT_CLIPPY_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

fn run_node_preflight(dir: &Path, dry_run: bool) -> bool {
    if dir.join("biome.json").exists() && which("biome") {
        if dry_run {
            eprintln!("    would run: biome check --write .");
        } else {
            eprintln!("    biome check --write...");
            run_in(dir, &["biome", "check", "--write", "."]);
        }
    } else if (dir.join(".prettierrc").exists()
        || dir.join(".prettierrc.json").exists()
        || dir.join("prettier.config.js").exists())
        && which("prettier")
    {
        if dry_run {
            eprintln!("    would run: prettier --write .");
        } else {
            eprintln!("    prettier --write...");
            run_in(dir, &["prettier", "--write", "."]);
        }
    }

    if (dir.join(".eslintrc").exists()
        || dir.join(".eslintrc.json").exists()
        || dir.join("eslint.config.js").exists()
        || dir.join("eslint.config.mjs").exists())
        && which("eslint")
    {
        if dry_run {
            eprintln!("    would run: eslint --fix .");
        } else {
            eprintln!("    eslint --fix...");
            run_in(dir, &["eslint", "--fix", "."]);
        }
    }

    true
}

fn stage_changes() {
    eprintln!("  [git] staging modified tracked files...");
    run_in(Path::new("."), &["git", "add", "-u"]);
}

fn run_in(dir: &Path, args: &[&str]) -> bool {
    Command::new(args[0])
        .args(&args[1..])
        .current_dir(dir)
        .stdin(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Spawn a command with a timeout. Returns:
/// - `Some(true)` on success
/// - `Some(false)` on non-zero exit
/// - `None` on timeout (process killed)
fn run_cmd_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<bool> {
    let mut child = match cmd.stdin(Stdio::null()).spawn() {
        Ok(c) => c,
        Err(_) => return Some(false),
    };

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait(); // reap zombie
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return Some(false),
        }
    }
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
