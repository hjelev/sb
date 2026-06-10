//! Path/entry classification helpers.
//!
//! A single home for the small filesystem-name and symlink checks that were
//! previously copy-pasted across the directory scanners, tree renderer and
//! status code.

use std::fs;
use std::path::Path;

/// Returns true if a file name marks a hidden entry (dotfile convention).
pub fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

/// Returns true if a directory entry is hidden (its name starts with `.`).
///
/// Centralizes the `entry.file_name().to_string_lossy().starts_with('.')`
/// pattern that the directory scanners and tree renderer all repeated.
pub fn is_hidden_entry(entry: &fs::DirEntry) -> bool {
    is_hidden_name(&entry.file_name().to_string_lossy())
}

/// Returns a directory entry's file name as a (lossy) `String`.
///
/// Replaces the repeated `entry.file_name().to_string_lossy().into_owned()`.
pub fn entry_name(entry: &fs::DirEntry) -> String {
    entry.file_name().to_string_lossy().into_owned()
}

/// Returns a path's final component as a (lossy) `String`, if it has one.
///
/// Replaces the repeated `path.file_name().map(|n| n.to_string_lossy().into_owned())`.
pub fn path_file_name(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

/// Returns a path's final component as a (lossy) `String`, falling back to the
/// whole path when it has no final component (e.g. `/`).
///
/// Replaces the repeated
/// `path.file_name().map(|n| n.to_string_lossy().into_owned())
///      .unwrap_or_else(|| path.to_string_lossy().into_owned())`.
pub fn display_name(path: &Path) -> String {
    path_file_name(path).unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Returns true if `path` itself is a symbolic link.
///
/// Uses `symlink_metadata()` so the link is inspected rather than its target;
/// any access error is treated as "not a symlink" (matching the previous
/// inline `.unwrap_or(false)` callers).
pub fn is_symlink<P: AsRef<Path>>(path: P) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_hidden_name() {
        assert!(is_hidden_name(".bashrc"));
        assert!(is_hidden_name(".sb"));
        assert!(!is_hidden_name("README.md"));
        assert!(!is_hidden_name("file."));
    }

    #[test]
    fn test_path_file_name() {
        assert_eq!(
            path_file_name(&PathBuf::from("/a/b/c.txt")),
            Some("c.txt".to_string())
        );
        assert_eq!(path_file_name(&PathBuf::from("/")), None);
    }

    #[test]
    fn test_display_name() {
        assert_eq!(display_name(&PathBuf::from("/a/b/c.txt")), "c.txt");
        // No final component → falls back to the whole path.
        assert_eq!(display_name(&PathBuf::from("/")), "/");
    }

    #[test]
    fn test_is_symlink_on_missing_path() {
        // A path that cannot be stat'd is reported as "not a symlink".
        assert!(!is_symlink("/tmp/nonexistent_path_12345_classify"));
    }
}
