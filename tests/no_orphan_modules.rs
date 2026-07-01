//! Guard against orphaned source files.
//!
//! This test walks `src/` and asserts every `.rs` file is reachable as a
//! module, so a future dropped `mod` declaration fails the build instead of
//! rotting silently.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Collect every `mod <name>;` declaration found anywhere under `src/`.
fn collect_mod_declarations(src: &Path) -> HashSet<String> {
    let mut decls = HashSet::new();
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                for line in text.lines() {
                    let line = line.trim();
                    // Match `mod foo;`, `pub mod foo;`, `pub(crate) mod foo;`
                    if let Some(rest) = line
                        .strip_prefix("pub(crate) mod ")
                        .or_else(|| line.strip_prefix("pub mod "))
                        .or_else(|| line.strip_prefix("mod "))
                        && let Some(name) = rest.strip_suffix(';') {
                            decls.insert(name.trim().to_string());
                        }
                }
            }
        }
    }
    decls
}

/// Collect every `.rs` source file that must be reachable through `mod`.
///
/// `main.rs` is the crate root and `mod.rs` files are reached via their
/// directory name, so both are excluded from the "needs a `mod foo;`" rule.
fn collect_source_files(src: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let name = path.file_name().unwrap().to_string_lossy();
                if name != "main.rs" && name != "mod.rs" {
                    files.push(path);
                }
            }
        }
    }
    files
}

#[test]
fn no_orphaned_source_files() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let decls = collect_mod_declarations(&src);

    let orphans: Vec<String> = collect_source_files(&src)
        .into_iter()
        .filter(|path| {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            !decls.contains(&stem)
        })
        .map(|path| path.display().to_string())
        .collect();

    assert!(
        orphans.is_empty(),
        "found source files not declared with `mod` (orphaned, not compiled):\n  {}",
        orphans.join("\n  ")
    );
}
