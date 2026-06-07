use std::{
    collections::HashSet,
    env, fs, io,
    path::PathBuf,
    process::Command,
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{Clear as TermClear, ClearType},
};
use crate::util::tui::{suspend_tui, resume_tui};
use ratatui::style::Color;

use crate::{App, AppMode, PathFilterMode, RemoteEntry, SshHost, SshMount};
use crate::ui;
use crate::util::cleanup::safe_cleanup_path;
use crate::util::command::CommandBuilder;

impl App {
    pub(crate) fn parse_ssh_config() -> Vec<SshHost> {
        let config_path = match env::var("HOME") {
            Ok(h) => PathBuf::from(h).join(".ssh/config"),
            Err(_) => return Vec::new(),
        };
        let content = match fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let mut hosts: Vec<SshHost> = Vec::new();
        let mut current: Option<SshHost> = None;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let sep = trimmed.find(|c: char| c.is_ascii_whitespace() || c == '=');
            let (raw_key, raw_val) = match sep {
                Some(pos) => (
                    &trimmed[..pos],
                    trimmed[pos + 1..]
                        .trim_start_matches(|c: char| c == '=' || c.is_ascii_whitespace()),
                ),
                None => (trimmed, ""),
            };
            let key = raw_key.to_lowercase();
            let val = raw_val.to_string();
            if key == "host" || key == "match" {
                if let Some(h) = current.take() {
                    if !h.alias.contains('*') && !h.alias.contains('?') {
                        hosts.push(h);
                    }
                }
                if key == "host" {
                    if let Some(alias) = val
                        .split_whitespace()
                        .find(|s| !s.contains('*') && !s.contains('?'))
                        .map(|s| s.to_string())
                    {
                        current = Some(SshHost {
                            hostname: alias.clone(),
                            alias,
                            user: None,
                            port: None,
                            identity_file: None,
                        });
                    }
                }
            } else if let Some(ref mut h) = current {
                match key.as_str() {
                    "hostname" => h.hostname = val,
                    "user" => h.user = Some(val),
                    "port" => h.port = val.parse().ok(),
                    "identityfile" => h.identity_file = Some(val),
                    _ => {}
                }
            }
        }
        if let Some(h) = current {
            if !h.alias.contains('*') && !h.alias.contains('?') {
                hosts.push(h);
            }
        }
        hosts
    }

    pub(crate) fn parse_rclone_remotes() -> Vec<RemoteEntry> {
        let out = match Command::new("rclone").args(["listremotes", "--long"]).output() {
            Ok(o) if o.status.success() => o,
            _ => return Vec::new(),
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ':');
                let name = parts.next()?.trim().to_string();
                let rtype = parts.next().unwrap_or("").trim().to_string();
                if name.is_empty() {
                    return None;
                }
                Some(RemoteEntry::Rclone { name, rtype })
            })
            .collect()
    }

    pub(crate) fn parse_local_mount_dirs() -> Vec<RemoteEntry> {
        let user = env::var("USER").unwrap_or_default();
        let uid = users::get_current_uid();
        let candidates: Vec<(&str, PathBuf)> = vec![
            ("media", PathBuf::from(format!("/media/{}", user))),
            ("run-media", PathBuf::from(format!("/run/media/{}", user))),
            ("mnt", PathBuf::from("/mnt")),
            ("gvfs", PathBuf::from(format!("/run/user/{}/gvfs", uid))),
        ];

        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut mounts: Vec<RemoteEntry> = Vec::new();

        for (source, root) in candidates {
            if !root.is_dir() {
                continue;
            }

            let entries = match fs::read_dir(&root) {
                Ok(rd) => rd,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() || !seen.insert(path.clone()) {
                    continue;
                }

                let child_name = entry.file_name().to_string_lossy().into_owned();
                let name = format!("{}:{}", source, child_name);
                mounts.push(RemoteEntry::LocalMount {
                    name,
                    mount_path: path,
                    source: source.to_string(),
                });
            }
        }

        mounts.sort_by(|a, b| a.alias().cmp(b.alias()));
        mounts
    }

    pub(crate) fn wait_for_mount_ready(path: &PathBuf) {
        for _ in 0..20 {
            let ready = Command::new("mountpoint")
                .args(["-q", path.to_string_lossy().as_ref()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ready {
                break;
            }
            thread::sleep(Duration::from_millis(120));
        }
    }

    pub(crate) fn refresh_remote_entries(&mut self) {
        let has_sshfs = self.integration_active("sshfs");
        let has_rclone = self.integration_active("rclone");
        let mut entries: Vec<RemoteEntry> = Vec::new();
        if has_sshfs {
            entries.extend(App::parse_ssh_config().into_iter().map(RemoteEntry::Ssh));
        }
        if has_rclone {
            entries.extend(App::parse_rclone_remotes());
        }
        entries.extend(self.archive_mounts.iter().map(|m| RemoteEntry::ArchiveMount {
            archive_name: m.archive_name.clone(),
            mount_path: m.mount_path.clone(),
        }));
        entries.extend(App::parse_local_mount_dirs());
        self.remote_entries = entries;
        if self.remote_entries.is_empty() {
            self.ssh_picker_selection = 0;
        } else {
            self.ssh_picker_selection =
                self.ssh_picker_selection.min(self.remote_entries.len() - 1);
        }
    }

    pub(crate) fn current_remote_mount(&self) -> Option<&SshMount> {
        self.ssh_mounts
            .iter()
            .filter(|mount| {
                self.current_dir == mount.mount_path || self.current_dir.starts_with(&mount.mount_path)
            })
            .max_by_key(|mount| mount.mount_path.components().count())
    }

    pub(crate) fn current_header_identity(&self, local_user: &str, local_host: &str) -> String {
        self.current_remote_mount()
            .map(|mount| mount.remote_label.clone())
            .unwrap_or_else(|| format!("{}@{}", local_user, local_host))
    }

    pub(crate) fn current_dir_display_path(&self) -> String {
        let Some(mount) = self.current_remote_mount() else {
            let path_str = self.current_dir.to_string_lossy().into_owned();
            if let Ok(home) = env::var("HOME") {
                if path_str == home {
                    return "~".to_string();
                }
                let home_prefix = format!("{}/", home);
                if let Some(rest) = path_str.strip_prefix(&home_prefix) {
                    return format!("~/{}", rest);
                }
            }
            return path_str;
        };

        let rel = self
            .current_dir
            .strip_prefix(&mount.mount_path)
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();

        if rel.is_empty() {
            return mount.remote_root.clone();
        }

        if mount.remote_root == "/" {
            format!("/{}", rel)
        } else if mount.remote_root.ends_with('/') {
            format!("{}{}", mount.remote_root, rel)
        } else {
            format!("{}/{}", mount.remote_root, rel)
        }
    }

    pub(crate) fn path_filter_suffix_text(&self) -> Option<String> {
        let filter = self.path_input_filter.as_ref()?;
        let suffix = match filter.mode {
            PathFilterMode::Prefix => format!("^{}", filter.pattern),
            PathFilterMode::Suffix => format!("{}$", filter.pattern),
            PathFilterMode::Contains => format!("~{}", filter.pattern),
        };
        Some(suffix)
    }

    pub(crate) fn path_with_filter_suffix(base: String, suffix: Option<String>) -> String {
        let Some(suffix) = suffix else {
            return base;
        };

        if base == "/" {
            format!("/{}", suffix)
        } else {
            format!("{}/{}", base, suffix)
        }
    }

    pub(crate) fn current_dir_display_path_with_filter(&self) -> String {
        Self::path_with_filter_suffix(self.current_dir_display_path(), self.path_filter_suffix_text())
    }

    pub(crate) fn current_path_edit_value(&self) -> String {
        let base = self.current_dir.to_string_lossy().into_owned();
        Self::path_with_filter_suffix(base, self.path_filter_suffix_text())
    }

    pub(crate) fn mount_rclone_remote(&mut self, name: &str, rtype: &str) -> io::Result<()> {
        if let Some(existing) = self.ssh_mounts.iter_mut().find(|m| m.host_alias == name) {
            existing.return_dir = self.current_dir.clone();
            let mount_path = existing.mount_path.clone();
            self.mode = AppMode::Browsing;
            self.try_enter_dir(mount_path);
            return Ok(());
        }
        let _ = rtype;
        let return_dir = self.current_dir.clone();
        let mount_dir = PathBuf::from(format!("/tmp/sbrs_rclone_{}", name));
        if mount_dir.exists() {
            let _ = safe_cleanup_path(&mount_dir);
        }
        fs::create_dir_all(&mount_dir)?;
        let remote_spec = format!("{}:", name);
        let status = Command::new("rclone")
            .args([
                "mount",
                &remote_spec,
                mount_dir.to_str().unwrap_or(""),
                "--daemon",
                "--vfs-cache-mode",
                "writes",
            ])
            .status()?;
        if status.success() {
            Self::wait_for_mount_ready(&mount_dir);
            let remote_os_icon = ui::icons::remote_os_nerd_icon(&mount_dir)
                .map(|(g, _)| (g, ui::theme::theme_spec(self.active_theme).icon_os));
            self.ssh_mounts.push(SshMount {
                host_alias: name.to_string(),
                mount_path: mount_dir.clone(),
                return_dir,
                remote_label: name.to_string(),
                remote_root: "/".to_string(),
                remote_os_icon,
            });
            self.mode = AppMode::Browsing;
            self.try_enter_dir(mount_dir);
            Ok(())
        } else {
            let _ = safe_cleanup_path(&mount_dir);
            Err(io::Error::other("rclone mount failed"))
        }
    }

    pub(crate) fn detect_ssh_remote_os_icon(host: &SshHost, theme_id: crate::ui::theme::ThemeId) -> Option<(&'static str, Color)> {
        let target = match &host.user {
            Some(u) => format!("{}@{}", u, host.hostname),
            None => host.hostname.clone(),
        };
        let mut cmd = Command::new("ssh");
        if let Some(port) = host.port {
            cmd.args(["-p", &port.to_string()]);
        }
        if let Some(idf) = &host.identity_file {
            let expanded = idf.replace('~', &env::var("HOME").unwrap_or_default());
            cmd.args(["-i", &expanded]);
        }
        let output = cmd.args([&target, "cat", "/etc/os-release"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let content = String::from_utf8_lossy(&output.stdout);
        ui::icons::os_nerd_icon_from_os_release_content(content.as_ref())
            .map(|(g, _)| (g, ui::theme::theme_spec(theme_id).icon_os))
    }

    pub(crate) fn mount_ssh_host(&mut self, host: &SshHost) -> io::Result<()> {
        if let Some(existing) = self
            .ssh_mounts
            .iter_mut()
            .find(|m| m.host_alias == host.alias)
        {
            existing.return_dir = self.current_dir.clone();
            if existing.remote_os_icon.is_none() {
                existing.remote_os_icon = Self::detect_ssh_remote_os_icon(host, self.active_theme);
            }
            let mount_path = existing.mount_path.clone();
            self.mode = AppMode::Browsing;
            self.try_enter_dir(mount_path);
            return Ok(());
        }
        let return_dir = self.current_dir.clone();
        let mount_dir = PathBuf::from(format!("/tmp/sbrs_sshfs_{}", host.alias));
        if mount_dir.exists() {
            let _ = safe_cleanup_path(&mount_dir);
        }
        fs::create_dir_all(&mount_dir)?;
        let remote_spec = match &host.user {
            Some(u) => format!("{}@{}:", u, host.hostname),
            None => format!("{}:", host.hostname),
        };
        let mut cmd = Command::new("sshfs");
        if let Some(port) = host.port {
            cmd.args(["-p", &port.to_string()]);
        }
        if let Some(idf) = &host.identity_file {
            let expanded = idf.replace('~', &env::var("HOME").unwrap_or_default());
            cmd.args(["-o", &format!("IdentityFile={}", expanded)]);
        }
        cmd.arg(&remote_spec).arg(&mount_dir);
        let status = cmd.status()?;
        if status.success() {
            Self::wait_for_mount_ready(&mount_dir);
            let remote_label = match &host.user {
                Some(user) => format!("{}@{}", user, host.hostname),
                None => host.hostname.clone(),
            };
            let remote_os_icon = ui::icons::remote_os_nerd_icon(&mount_dir)
                .map(|(g, _)| (g, ui::theme::theme_spec(self.active_theme).icon_os))
                .or_else(|| Self::detect_ssh_remote_os_icon(host, self.active_theme));
            self.ssh_mounts.push(SshMount {
                host_alias: host.alias.clone(),
                mount_path: mount_dir.clone(),
                return_dir,
                remote_label,
                remote_root: "~".to_string(),
                remote_os_icon,
            });
            self.mode = AppMode::Browsing;
            self.try_enter_dir(mount_dir);
            Ok(())
        } else {
            let _ = safe_cleanup_path(&mount_dir);
            Err(io::Error::other("sshfs mount failed"))
        }
    }

    pub(crate) fn try_leave_ssh_mount(&mut self) -> bool {
        let mount_idx = self
            .ssh_mounts
            .iter()
            .rposition(|m| self.current_dir == m.mount_path);
        let Some(idx) = mount_idx else {
            return false;
        };
        self.remember_current_selection();
        let return_dir = self.ssh_mounts[idx].return_dir.clone();
        self.current_dir = return_dir;
        self.refresh_entries_or_status();
        true
    }

    pub(crate) fn cleanup_ssh_mounts(&mut self) {
        for mount in &self.ssh_mounts {
            if self.current_dir == mount.mount_path || self.current_dir.starts_with(&mount.mount_path)
            {
                self.current_dir = mount.return_dir.clone();
                break;
            }
        }
        while let Some(mount) = self.ssh_mounts.pop() {
            let _ = CommandBuilder::unmount_archive(&mount.mount_path);
            let _ = safe_cleanup_path(&mount.mount_path);
        }
    }

    pub(crate) fn unmount_ssh_mount_by_alias(&mut self, alias: &str) -> bool {
        let Some(idx) = self.ssh_mounts.iter().rposition(|m| m.host_alias == alias) else {
            return false;
        };

        let mount = self.ssh_mounts.remove(idx);
        if self.current_dir == mount.mount_path || self.current_dir.starts_with(&mount.mount_path) {
            self.current_dir = mount.return_dir.clone();
            self.refresh_entries_or_status();
        }

        let _ = CommandBuilder::unmount_archive(&mount.mount_path);
        let _ = safe_cleanup_path(&mount.mount_path);
        true
    }

    pub(crate) fn open_ssh_shell_session(&mut self, host: &SshHost) -> io::Result<()> {
        suspend_tui()?;
        execute!(io::stdout(), Show)?;

        let mut cmd = Command::new("ssh");
        if let Some(port) = host.port {
            cmd.args(["-p", &port.to_string()]);
        }
        if let Some(idf) = &host.identity_file {
            let expanded = idf.replace('~', &env::var("HOME").unwrap_or_default());
            cmd.args(["-i", &expanded]);
        }
        cmd.arg(&host.alias);

        let status = cmd.status();

        resume_tui()?;
        execute!(io::stdout(), TermClear(ClearType::All), MoveTo(0, 0))?;
        execute!(io::stdout(), Hide)?;

        match status {
            Ok(exit_status) => {
                if exit_status.success() {
                    self.set_status(format!("SSH session closed: {}", host.alias));
                } else if let Some(code) = exit_status.code() {
                    self.set_status(format!("ssh exited with code {} for {}", code, host.alias));
                } else {
                    self.set_status(format!("ssh session ended for {}", host.alias));
                }
            }
            Err(e) => {
                self.set_status(format!("failed to start ssh session for {}: {}", host.alias, e));
            }
        }

        self.refresh_entries_or_status();
        Ok(())
    }
}
