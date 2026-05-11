//! Safe file cleanup utilities that return proper errors instead of silently failing.
//!
//! Replaces patterns like `let _ = fs::remove_file(path)` with functions that
//! return `Result` for proper error handling.
#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::io;

/// Safely remove a file, returning an error if the operation fails.
///
/// Replaces silent cleanup patterns like `let _ = fs::remove_file(path)`.
///
/// # Arguments
/// * `path` - Path to the file to remove
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` with error if removal fails (file not found is OK)
///
/// # Example
/// ```ignore
/// safe_cleanup_path(&temp_file)?;
/// ```
pub fn safe_cleanup_path<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();

    // Try removing as file first (most common case)
    match fs::remove_file(path) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File doesn't exist, which is fine for cleanup operations
            Ok(())
        }
        Err(_) => {
            // May be a directory — try remove_dir_all (also handles NotFound)
            match fs::remove_dir_all(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }
    }
}

/// Safely remove a directory, handling both files and directories.
///
/// Attempts to remove the path regardless of whether it's a file or directory.
///
/// # Arguments
/// * `path` - Path to remove
///
/// # Returns
/// * `Ok(())` on success or if path doesn't exist
/// * `Err` if removal fails
pub fn safe_unlink_path<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();

    // First try remove_file (works for symlinks too)
    match fs::remove_file(path) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Doesn't exist - that's OK for cleanup
            Ok(())
        }
        Err(_) => {
            // May be a directory — try remove_dir_all
            match fs::remove_dir_all(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_safe_cleanup_existing_file() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let file_path = tmpdir.path().join("test.txt");
        File::create(&file_path)?;

        assert!(file_path.exists());
        safe_cleanup_path(&file_path)?;
        assert!(!file_path.exists());
        Ok(())
    }

    #[test]
    fn test_safe_cleanup_nonexistent_file() -> Result<()> {
        // Should not error if file doesn't exist
        safe_cleanup_path("/tmp/this_file_does_not_exist_12345")?;
        Ok(())
    }

    #[test]
    fn test_safe_cleanup_directory() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let subdir = tmpdir.path().join("subdir");
        fs::create_dir(&subdir)?;
        File::create(subdir.join("file.txt"))?;

        assert!(subdir.exists());
        safe_cleanup_path(&subdir)?;
        assert!(!subdir.exists());
        Ok(())
    }
}
