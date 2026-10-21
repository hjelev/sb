//! Private temp paths. Everything sb writes under the shared temp dir goes
//! through here so names are unpredictable and permissions are owner-only
//! (`0600` files, `0700` dirs): another local user can neither pre-create /
//! symlink the path nor read what we put there (decrypted `age` plaintext,
//! clipboard contents, ...).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Create an empty owner-only file `<tmp>/<prefix>XXXXXX<suffix>` and return
/// its path. The caller owns cleanup.
pub fn private_file(prefix: &str, suffix: &str) -> io::Result<PathBuf> {
    tempfile::Builder::new()
        .prefix(prefix)
        .suffix(suffix)
        .tempfile()?
        .into_temp_path()
        .keep()
        .map_err(|e| e.error)
}

/// Create an empty owner-only directory `<tmp>/<prefix>XXXXXX` and return its
/// path. The caller owns cleanup.
pub fn private_dir(prefix: &str) -> io::Result<PathBuf> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix);
    // tempfile creates dirs with the umask-derived mode (usually 0775), so ask
    // for 0700 explicitly; it's applied at mkdir time, not chmod'd after.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    Ok(builder.tempdir()?.keep())
}

static SESSION_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Owner-only scratch directory shared for the lifetime of this process, for
/// derived files that are cached by name (e.g. rendered SVGs). Removed by
/// [`cleanup_session_cache`] on exit. Only plain files belong here — never
/// mount points.
pub fn session_cache_dir() -> Option<&'static Path> {
    SESSION_CACHE
        .get_or_init(|| private_dir("sbrs_cache_").ok())
        .as_deref()
}

pub fn cleanup_session_cache() {
    if let Some(Some(dir)) = SESSION_CACHE.get() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_paths_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let file = private_file("sbrs_test_", ".txt").unwrap();
        let dir = private_dir("sbrs_test_").unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(
            file.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".txt")
        );
        let _ = std::fs::remove_file(file);
        let _ = std::fs::remove_dir(dir);
    }
}
