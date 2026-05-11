//! Unified path metadata classification.
//!
//! Replaces scattered patterns of `fs::symlink_metadata()` + `is_dir()` + `is_symlink()`
//! checks across the codebase with a single source of truth.
#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::io;

/// Result of classifying a path.
#[derive(Debug, Clone, Copy)]
pub struct PathClass {
    /// Whether the path (dereferenced) is a directory
    pub is_dir: bool,
    /// Whether the path is a symbolic link
    pub is_symlink: bool,
    /// File size in bytes
    pub size: u64,
}

/// Classify a path into its type (file, dir, symlink).
///
/// Uses `symlink_metadata()` to avoid following symlinks, then checks properties.
///
/// # Arguments
/// * `path` - Path to classify
///
/// # Returns
/// * `Ok(PathClass)` with classification
/// * `Err` if path doesn't exist or can't be accessed
///
/// # Example
/// ```ignore
/// let class = classify_path(&my_path)?;
/// if class.is_dir {
///     println!("Directory");
/// } else if class.is_symlink {
///     println!("Symlink");
/// } else {
///     println!("Regular file");
/// }
/// ```
pub fn classify_path<P: AsRef<Path>>(path: P) -> io::Result<PathClass> {
    let path = path.as_ref();
    let meta = fs::symlink_metadata(path)?;

    Ok(PathClass {
        is_dir: meta.is_dir(),
        is_symlink: meta.is_symlink(),
        size: meta.len(),
    })
}

/// Get metadata for a path, following symlinks (standard `metadata()`).
///
/// Use this when you want the target of a symlink's metadata.
/// Use `classify_path()` when you want to know if something IS a symlink.
pub fn get_metadata<P: AsRef<Path>>(path: P) -> io::Result<fs::Metadata> {
    fs::metadata(path.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_classify_regular_file() -> io::Result<()> {
        let tmpdir = TempDir::new()?;
        let file_path = tmpdir.path().join("test.txt");
        File::create(&file_path)?;

        let class = classify_path(&file_path)?;
        assert!(!class.is_dir);
        assert!(!class.is_symlink);
        Ok(())
    }

    #[test]
    fn test_classify_directory() -> io::Result<()> {
        let tmpdir = TempDir::new()?;
        let subdir = tmpdir.path().join("subdir");
        fs::create_dir(&subdir)?;

        let class = classify_path(&subdir)?;
        assert!(class.is_dir);
        assert!(!class.is_symlink);
        Ok(())
    }

    #[test]
    fn test_classify_nonexistent() {
        let class = classify_path("/tmp/nonexistent_path_12345");
        assert!(class.is_err());
    }
}
