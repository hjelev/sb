use std::fs;
use std::path::Path;
use std::io;

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
