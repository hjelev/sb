use std::{collections::HashMap, fs};

use crate::App;

impl App {
    pub(crate) fn parse_permissions(meta: &fs::Metadata) -> String {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode();
            let mut p = String::with_capacity(10);
            p.push(if meta.is_dir() { 'd' } else { '-' });
            let chars = ['r', 'w', 'x'];
            for i in (0..9).rev() {
                if mode & (1 << i) != 0 {
                    p.push(chars[2 - (i % 3)]);
                } else {
                    p.push('-');
                }
            }
            p
        }
        #[cfg(not(unix))]
        {
            "----------".to_string()
        }
    }

    pub(crate) fn build_uid_cache(entries: &[fs::DirEntry]) -> HashMap<u32, String> {
        let refs: Vec<&fs::DirEntry> = entries.iter().collect();
        Self::build_uid_cache_refs(&refs)
    }

    pub(crate) fn build_uid_cache_refs(entries: &[&fs::DirEntry]) -> HashMap<u32, String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mut map: HashMap<u32, String> = HashMap::new();
            for entry in entries {
                if let Ok(meta) = entry.metadata() {
                    let uid = meta.uid();
                    map.entry(uid).or_insert_with(|| {
                        users::get_user_by_uid(uid)
                            .map(|u| u.name().to_string_lossy().into_owned())
                            .unwrap_or_else(|| uid.to_string())
                    });
                }
            }
            map
        }
        #[cfg(not(unix))]
        {
            HashMap::new()
        }
    }

    pub(crate) fn build_gid_cache(entries: &[fs::DirEntry]) -> HashMap<u32, String> {
        let refs: Vec<&fs::DirEntry> = entries.iter().collect();
        Self::build_gid_cache_refs(&refs)
    }

    pub(crate) fn build_gid_cache_refs(entries: &[&fs::DirEntry]) -> HashMap<u32, String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mut map: HashMap<u32, String> = HashMap::new();
            for entry in entries {
                if let Ok(meta) = entry.metadata() {
                    let gid = meta.gid();
                    map.entry(gid).or_insert_with(|| {
                        users::get_group_by_gid(gid)
                            .map(|g| g.name().to_string_lossy().into_owned())
                            .unwrap_or_else(|| gid.to_string())
                    });
                }
            }
            map
        }
        #[cfg(not(unix))]
        {
            HashMap::new()
        }
    }

    pub(crate) fn format_size(bytes: u64) -> String {
        crate::util::format::format_size(bytes)
    }

    pub(crate) fn toggle_executable_permissions(&mut self) {
        #[cfg(not(unix))]
        {
            self.set_status("executable permission toggle is only supported on Unix");
            return;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let targets = self.delete_targets();
            if targets.is_empty() {
                self.set_status("no selected item");
                return;
            }

            let mut changed = 0usize;
            let mut skipped_dirs = 0usize;
            let mut failed = 0usize;

            for path in targets {
                let meta = match fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => {
                        failed += 1;
                        continue;
                    }
                };

                if meta.is_dir() {
                    skipped_dirs += 1;
                    continue;
                }

                let mode = meta.permissions().mode();
                let new_mode = if mode & 0o111 != 0 {
                    mode & !0o111
                } else {
                    mode | 0o111
                };

                let mut perms = meta.permissions();
                perms.set_mode(new_mode);
                if fs::set_permissions(&path, perms).is_ok() {
                    changed += 1;
                } else {
                    failed += 1;
                }
            }

            if changed > 0 {
                self.refresh_entries_or_status();
            }

            if changed > 0 && failed == 0 && skipped_dirs == 0 {
                self.set_status(format!("toggled executable bit on {} file(s)", changed));
            } else if changed > 0 {
                self.set_status(format!(
                    "toggled {} file(s), skipped {} dir(s), {} failed",
                    changed, skipped_dirs, failed
                ));
            } else if skipped_dirs > 0 && failed == 0 {
                self.set_status("no files changed (directories skipped)");
            } else {
                self.set_status("failed to toggle executable permissions");
            }
        }
    }
}
