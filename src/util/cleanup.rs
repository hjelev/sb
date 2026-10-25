use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Remove a path (file or directory) ignoring NotFound; propagate all other errors.
pub fn safe_cleanup_path<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    match fs::remove_file(path) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {}
    }
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// FUSE mount points this process currently owns (archives, rclone/sshfs).
///
/// Mirrors `App::archive_mounts` / `remote.ssh_mounts` so the panic hook —
/// which has no access to `App` and runs right before `panic = "abort"` kills
/// the process — can still unmount them instead of leaving stale mounts.
static ACTIVE_MOUNTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub fn register_mount(path: &Path) {
    ACTIVE_MOUNTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(path.to_path_buf());
}

pub fn unregister_mount(path: &Path) {
    ACTIVE_MOUNTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|p| p != path);
}

/// Emergency unmount of everything still registered. Only for the panic hook:
/// uses `try_lock` so a panic raised while the lock is held can't deadlock.
pub fn unmount_registered_on_crash() {
    let Ok(mut mounts) = ACTIVE_MOUNTS.try_lock() else {
        return;
    };
    while let Some(path) = mounts.pop() {
        let _ = crate::util::command::CommandBuilder::unmount_archive(&path);
        let _ = fs::remove_dir(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_unregister_mounts() {
        let p = Path::new("/nonexistent/sbrs_test_mount");
        register_mount(p);
        assert!(ACTIVE_MOUNTS.lock().unwrap().iter().any(|m| m == p));
        unregister_mount(p);
        assert!(!ACTIVE_MOUNTS.lock().unwrap().iter().any(|m| m == p));
    }
}
