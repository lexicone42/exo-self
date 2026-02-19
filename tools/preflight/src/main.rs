//! preflight — run formatters + linters + stage before committing.
//!
//! Detects project type from files in the current directory (or git root)
//! and runs the appropriate fixers. Stages any modified files afterward
//! so the next `git commit` lands clean on the first try.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

struct Project {
    kind: &'static str,
    path: PathBuf,
}

fn main() -> ExitCode {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        eprintln!("Pre-commit preflight: run formatters + linters + auto-fix + stage changes");
        eprintln!();
        eprintln!("Usage: preflight [--dry-run]");
        eprintln!();
        eprintln!(
            "Detects Python (ruff), Rust (cargo fmt + clippy), and Node (biome/prettier/eslint)"
        );
        eprintln!("projects and runs fixers automatically. Stages modified files so the next git");
        eprintln!("commit lands clean on the first try. Use before committing to avoid the");
        eprintln!("fail-restage-recommit cycle.");
        return ExitCode::SUCCESS;
    }

    let dry_run = std::env::args().any(|a| a == "--dry-run" || a == "-n");

    let projects = detect_projects();

    if projects.is_empty() {
        eprintln!("preflight: no recognized project files found");
        return ExitCode::from(1);
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
        ExitCode::from(1)
    } else {
        eprintln!("preflight: all checks passed");
        ExitCode::SUCCESS
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
        // Skip if this is a subcrate (has a workspace parent)
        // by checking if there's a [workspace] in the toml
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

    if which("cargo") {
        if dry_run {
            eprintln!("    would run: cargo fmt");
            eprintln!(
                "    would run: cargo clippy --fix --allow-dirty --allow-staged --all-targets"
            );
            eprintln!("    would run: cargo fmt (second pass — clippy --fix can break formatting)");
        } else {
            // Pass 1: fmt to normalize formatting before clippy
            eprintln!("    cargo fmt...");
            if !run_in(dir, &["cargo", "fmt"]) {
                ok = false;
            }
            // Pass 2: clippy --fix (may rewrite code in non-fmt-compliant ways)
            eprintln!("    cargo clippy --fix...");
            if !run_in(
                dir,
                &[
                    "cargo",
                    "clippy",
                    "--fix",
                    "--allow-dirty",
                    "--allow-staged",
                    "--all-targets",
                ],
            ) {
                eprintln!("    clippy found unfixable issues (fixes still applied)");
            }
            // Pass 3: fmt again to clean up clippy's rewrites
            eprintln!("    cargo fmt (post-clippy cleanup)...");
            if !run_in(dir, &["cargo", "fmt"]) {
                ok = false;
            }
        }
    } else {
        eprintln!("    cargo not found, skipping");
    }

    ok
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
