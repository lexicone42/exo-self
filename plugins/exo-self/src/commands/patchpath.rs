//! patchpath — resolve the correct `mock.patch()` target for a Python symbol.
//!
//! Python's mock.patch() requires you to patch where a name is *looked up*,
//! not where it's *defined*. This tool traces the import chain to find the
//! correct patch target.

use std::path::{Path, PathBuf};

#[derive(Debug)]
struct ImportEntry {
    symbol: String,
    source_module: String,
    importing_module: String,
}

pub fn run(symbol: &str, module: Option<&str>) {
    let py_files = find_python_files(Path::new("."));
    let import_index = build_import_index(&py_files);

    if let Some(target_module) = module {
        resolve_patch_path(symbol, target_module, &import_index);
    } else {
        print_all_importers(symbol, &import_index);
    }
}

fn resolve_patch_path(symbol: &str, target_module: &str, import_index: &[ImportEntry]) {
    let matches: Vec<&ImportEntry> = import_index
        .iter()
        .filter(|e| e.importing_module == target_module && e.symbol == symbol)
        .collect();

    if matches.is_empty() {
        let module_imports: Vec<&ImportEntry> = import_index
            .iter()
            .filter(|e| e.importing_module == target_module && e.source_module.ends_with(symbol))
            .collect();

        if module_imports.is_empty() {
            eprintln!("Symbol `{symbol}` not found in imports of `{target_module}`.");
            eprintln!();
            eprintln!("Possible reasons:");
            eprintln!(
                "  - The symbol is defined in {target_module} itself (patch: {target_module}.{symbol})"
            );
            eprintln!("  - The symbol is imported dynamically");
            eprintln!("  - The module path is wrong");
            eprintln!();
            print_all_importers(symbol, import_index);
        } else {
            eprintln!("Symbol `{symbol}` appears to be accessed via module attribute.");
            eprintln!("In this case, patch at the source:");
            for entry in &module_imports {
                println!("{}.{symbol}", entry.source_module);
            }
        }
    } else if matches.len() == 1 {
        let patch_target = format!("{target_module}.{symbol}");
        println!("{patch_target}");
        eprintln!();
        eprintln!(
            "# {target_module} imports {symbol} from {}",
            matches[0].source_module
        );
        eprintln!("# Correct mock.patch() target: \"{patch_target}\"");
        eprintln!(
            "# Wrong (would patch the source): \"{}.{symbol}\"",
            matches[0].source_module
        );
    } else {
        eprintln!("Multiple imports of `{symbol}` found in `{target_module}`:");
        for entry in &matches {
            let patch_target = format!("{target_module}.{symbol}");
            println!("{patch_target}");
            eprintln!("  from {} import {symbol}", entry.source_module);
        }
    }
}

fn print_all_importers(symbol: &str, import_index: &[ImportEntry]) {
    let importers: Vec<&ImportEntry> = import_index.iter().filter(|e| e.symbol == symbol).collect();

    if importers.is_empty() {
        eprintln!("No modules import `{symbol}`.");
    } else {
        eprintln!("Modules that import `{symbol}`:");
        for entry in &importers {
            eprintln!(
                "  {}.{symbol}  (from {})",
                entry.importing_module, entry.source_module
            );
        }
    }
}

fn build_import_index(files: &[PathBuf]) -> Vec<ImportEntry> {
    let mut entries = Vec::new();
    for file in files {
        let module_path = file_to_module(file);
        if let Ok(content) = std::fs::read_to_string(file) {
            parse_imports(&content, &module_path, &mut entries);
        }
    }
    entries
}

fn parse_imports(content: &str, module_path: &str, entries: &mut Vec<ImportEntry>) {
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("from ")
            && let Some((source, import_part)) = rest.split_once(" import ")
        {
            let source = source.trim();
            let resolved_source = resolve_relative_import(source, module_path);

            let mut full_import = import_part.to_string();
            if full_import.contains('(') && !full_import.contains(')') {
                for continuation in lines.by_ref() {
                    full_import.push(' ');
                    full_import.push_str(continuation.trim());
                    if continuation.contains(')') {
                        break;
                    }
                }
            }

            while full_import.ends_with('\\') {
                full_import.pop();
                if let Some(continuation) = lines.next() {
                    full_import.push(' ');
                    full_import.push_str(continuation.trim());
                }
            }

            let clean = full_import
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim();

            if clean == "*" {
                continue;
            }

            for item in clean.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }

                let local_name = if let Some((_original, alias)) = item.split_once(" as ") {
                    alias.trim()
                } else {
                    item
                };

                entries.push(ImportEntry {
                    symbol: local_name.to_string(),
                    source_module: resolved_source.clone(),
                    importing_module: module_path.to_string(),
                });
            }
        }
    }
}

fn resolve_relative_import(source: &str, current_module: &str) -> String {
    if !source.starts_with('.') {
        return source.to_string();
    }

    let parts: Vec<&str> = current_module.split('.').collect();
    let dot_count = source.chars().take_while(|c| *c == '.').count();
    let remainder = &source[dot_count..];

    let base_parts = if parts.len() > dot_count {
        &parts[..parts.len() - dot_count]
    } else {
        &[]
    };

    let mut resolved = base_parts.join(".");
    if !remainder.is_empty() {
        if !resolved.is_empty() {
            resolved.push('.');
        }
        resolved.push_str(remainder);
    }

    resolved
}

fn file_to_module(path: &Path) -> String {
    let s = path.to_str().unwrap_or("");
    let s = s.strip_prefix("./").unwrap_or(s);
    let s = s.strip_suffix(".py").unwrap_or(s);
    let s = s.strip_suffix("/__init__").unwrap_or(s);
    let module = s.replace('/', ".");
    module.strip_prefix("src.").unwrap_or(&module).to_string()
}

fn find_python_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_dir(root, &mut files);
    files
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if name.starts_with('.')
            || name == "node_modules"
            || name == "__pycache__"
            || name == ".venv"
            || name == "venv"
            || name == ".tox"
            || name == "target"
            || name == "cdk.out"
            || name == "dist"
            || name == "build"
            || name == "_build"
            || name == "site-packages"
        {
            continue;
        }

        if path.is_dir() {
            walk_dir(&path, files);
        } else if name.ends_with(".py") {
            files.push(path);
        }
    }
}
