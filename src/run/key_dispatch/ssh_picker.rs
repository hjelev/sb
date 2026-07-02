use super::*;
use crate::util::tui::{resume_tui, suspend_tui};

pub(crate) fn handle_ssh_picker_key(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    key: KeyEvent,
) -> io::Result<KeyDispatchOutcome> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => { app.mode = AppMode::Browsing; }
        KeyCode::BackTab => {
            app.panel_tab = 2;
            app.refresh_bookmarks_cache();
            app.mode = AppMode::Bookmarks;
        }
        KeyCode::Tab => {
            app.begin_sort_menu();
        }
        KeyCode::Up => {
            if app.ssh_picker_selection > 0 {
                app.ssh_picker_selection -= 1;
            }
        }
        KeyCode::Down => {
            if !app.remote_entries.is_empty() && app.ssh_picker_selection < app.remote_entries.len() - 1 {
                app.ssh_picker_selection += 1;
            }
        }
        KeyCode::Enter | KeyCode::Right => {
            if let Some(entry) = app.remote_entries.get(app.ssh_picker_selection).cloned() {
                let alias = entry.alias().to_string();
                match entry {
                    RemoteEntry::Ssh(host) => {
                        let already_mounted = app.ssh_mounts.iter().any(|m| m.host_alias == alias);
                        if already_mounted {
                            app.mount_ssh_host(&host)?;
                        } else {
                            suspend_tui()?;
                            let result = app.mount_ssh_host(&host);
                            resume_tui()?;
                            terminal.clear()?;
                            if result.is_err() {
                                app.set_status(format!("Failed to mount {}", alias));
                                app.mode = AppMode::Browsing;
                            }
                        }
                    }
                    RemoteEntry::Rclone { name, rtype } => {
                        let already_mounted = app.ssh_mounts.iter().any(|m| m.host_alias == alias);
                        if already_mounted {
                            app.mount_rclone_remote(&name, &rtype)?;
                        } else {
                            suspend_tui()?;
                            println!("Connecting to rclone remote: {}…", name);
                            let result = app.mount_rclone_remote(&name, &rtype);
                            resume_tui()?;
                            terminal.clear()?;
                            if result.is_err() {
                                app.set_status(format!("Failed to mount rclone remote {}", name));
                                app.mode = AppMode::Browsing;
                            }
                        }
                    }
                    RemoteEntry::ArchiveMount { mount_path, archive_name } => {
                        if mount_path.is_dir() {
                            app.mode = AppMode::Browsing;
                            app.try_enter_dir_on_active_panel(mount_path);
                        } else {
                            app.set_status(format!("mount not available: {}", archive_name));
                            app.mode = AppMode::Browsing;
                        }
                    }
                    RemoteEntry::LocalMount { mount_path, name, .. } => {
                        if mount_path.is_dir() {
                            app.mode = AppMode::Browsing;
                            app.try_enter_dir_on_active_panel(mount_path);
                        } else {
                            app.set_status(format!("mount not available: {}", name));
                            app.mode = AppMode::Browsing;
                        }
                    }
                }
            }
        }
        KeyCode::Char('u') | KeyCode::Delete => {
            if let Some(entry) = app.remote_entries.get(app.ssh_picker_selection).cloned() {
                match entry {
                    RemoteEntry::Ssh(host) => {
                        if app.unmount_ssh_mount_by_alias(&host.alias) {
                            app.set_status(format!("unmounted {}", host.alias));
                        } else {
                            app.set_status(format!("not mounted: {}", host.alias));
                        }
                    }
                    RemoteEntry::Rclone { name, .. } => {
                        if app.unmount_ssh_mount_by_alias(&name) {
                            app.set_status(format!("unmounted {}", name));
                        } else {
                            app.set_status(format!("not mounted: {}", name));
                        }
                    }
                    RemoteEntry::ArchiveMount { mount_path, archive_name } => {
                        if app.unmount_archive_mount_by_path(&mount_path) {
                            app.set_status(format!("unmounted {}", archive_name));
                        } else {
                            app.set_status(format!("not mounted: {}", archive_name));
                        }
                    }
                    RemoteEntry::LocalMount { name, .. } => {
                        app.set_status(format!("external mount: {} (unmount outside sb)", name));
                    }
                }

                app.refresh_remote_entries();
            }
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            if let Some(entry) = app.remote_entries.get(app.ssh_picker_selection).cloned() {
                match entry {
                    RemoteEntry::Ssh(host) => {
                        app.open_ssh_shell_session(&host)?;
                        terminal.clear()?;
                    }
                    _ => {
                        app.set_status("'s' is available only for SSH hosts");
                    }
                }
            }
        }
        _ => {}
    }
    Ok(KeyDispatchOutcome::Ok)
}

