use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};

use crate::util::tui::{resume_tui, resume_tui_cleared, suspend_tui};
use crate::{App, ArchiveKind, ZIP_BASED_EXTENSIONS};

/// Tar-family suffixes (including compressed variants) recognized as archives.
const TAR_EXTENSIONS: &[&str] = &[
    ".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz", ".tbz2", ".tar.xz", ".txz", ".tar.zst",
    ".tzst",
];

/// Returns true if a (lowercased) file name ends with a tar-family suffix.
fn is_tar_like(lower_name: &str) -> bool {
    TAR_EXTENSIONS.iter().any(|ext| lower_name.ends_with(ext))
}

impl App {
    pub(crate) fn is_supported_archive(path: &PathBuf) -> bool {
        let lower_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        let tar_like = is_tar_like(&lower_name);

        let seven_zip = lower_name.ends_with(".7z");
        let rar = lower_name.ends_with(".rar");

        let ext_supported = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ZIP_BASED_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false);

        ext_supported || tar_like || seven_zip || rar || Self::has_zip_signature(path)
    }

    pub(crate) fn is_fuse_zip_archive(path: &PathBuf) -> bool {
        matches!(Self::archive_kind(path), Some(ArchiveKind::Zip))
    }

    pub(crate) fn is_archivemount_archive(path: &PathBuf) -> bool {
        matches!(Self::archive_kind(path), Some(ArchiveKind::Tar) | Some(ArchiveKind::Zip))
    }

    pub(crate) fn archive_kind(path: &PathBuf) -> Option<ArchiveKind> {
        let lower_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        let is_zip = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ZIP_BASED_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
            || Self::has_zip_signature(path);
        if is_zip {
            return Some(ArchiveKind::Zip);
        }

        if is_tar_like(&lower_name) {
            return Some(ArchiveKind::Tar);
        }
        if lower_name.ends_with(".7z") {
            return Some(ArchiveKind::SevenZip);
        }
        if lower_name.ends_with(".rar") {
            return Some(ArchiveKind::Rar);
        }
        None
    }

    pub(crate) fn is_image_file(path: &Path) -> bool {
        const IMAGE_EXTENSIONS: &[&str] = &[
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "avif", "heic", "ico",
        ];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| IMAGE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn is_svg_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("svg"))
            .unwrap_or(false)
    }

    pub(crate) fn is_audio_file(path: &Path) -> bool {
        const AUDIO_EXTENSIONS: &[&str] = &[
            "mp3", "flac", "wav", "ogg", "opus", "m4a", "aac", "wma", "aiff", "aif", "alac", "mid", "midi",
        ];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| AUDIO_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn is_json_file(path: &Path) -> bool {
        const JSON_EXTENSIONS: &[&str] = &["json", "jsonc", "jsonl", "ndjson", "geojson"];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| JSON_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn is_markdown_file(path: &Path) -> bool {
        const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mkd", "mkdn"];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| MARKDOWN_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn is_html_file(path: &Path) -> bool {
        const HTML_EXTENSIONS: &[&str] = &["html", "htm", "xhtml"];
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| HTML_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn is_mermaid_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("mmd"))
            .unwrap_or(false)
    }

    pub(crate) fn is_pdf_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false)
    }

    pub(crate) fn is_cast_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("cast"))
            .unwrap_or(false)
    }

    pub(crate) fn is_age_protected_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("age"))
            .unwrap_or(false)
    }

    pub(crate) fn age_protected_output_path(path: &Path) -> PathBuf {
        PathBuf::from(format!("{}.age", path.to_string_lossy()))
    }

    pub(crate) fn age_plain_output_path(path: &PathBuf) -> PathBuf {
        let mut out = path.clone();
        out.set_extension("");
        if out == *path {
            path.with_extension("decrypted")
        } else {
            out
        }
    }

    pub(crate) fn age_temp_decrypt_paths(path: &PathBuf, purpose: &str) -> io::Result<(PathBuf, PathBuf)> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_dir = env::temp_dir().join(format!(
            "sbrs_age_{}_{}_{}",
            purpose,
            std::process::id(),
            stamp
        ));
        fs::create_dir_all(&tmp_dir)?;

        let plain_name = Self::age_plain_output_path(path)
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_else(|| "decrypted.bin".into());
        let tmp_path = tmp_dir.join(plain_name);
        Ok((tmp_dir, tmp_path))
    }

    pub(crate) fn is_delimited_text_file(path: &Path) -> bool {
        const DELIMITED_EXTENSIONS: &[&str] = &["csv", "tsv", "tab", "psv", "dsv", "ssv"];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| DELIMITED_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn is_sqlite_db_file(path: &Path) -> bool {
        const SQLITE_EXTENSIONS: &[&str] = &[
            "db",
            "sqlite",
            "sqlite3",
            "db3",
            "s3db",
            "sl3",
        ];

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| SQLITE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    }

    pub(crate) fn is_binary_file(path: &PathBuf) -> bool {
        let Ok(mut file) = fs::File::open(path) else {
            return false;
        };
        let mut buf = [0u8; 8192];
        let Ok(n) = file.read(&mut buf) else {
            return false;
        };
        buf[..n].contains(&0u8)
    }

    pub(crate) fn has_zip_signature(path: &PathBuf) -> bool {
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(_) => return false,
        };

        let mut magic = [0u8; 4];
        match file.read(&mut magic) {
            Ok(read) if read >= 4 => {
                magic == [0x50, 0x4B, 0x03, 0x04]
                    || magic == [0x50, 0x4B, 0x05, 0x06]
                    || magic == [0x50, 0x4B, 0x07, 0x08]
            }
            _ => false,
        }
    }
}

impl App {
    pub(crate) fn age_encrypt_file_interactive(input: &PathBuf, output: &PathBuf) -> Result<(), String> {
        let status = Command::new("age")
            .args(["-p", "-o"])
            .arg(output)
            .arg(input)
            .status()
            .map_err(|e| e.to_string())?;

        if status.success() {
            Ok(())
        } else {
            Err("age encryption failed".to_string())
        }
    }

    pub(crate) fn age_decrypt_file_interactive(input: &PathBuf, output: &PathBuf) -> Result<(), String> {
        let status = Command::new("age")
            .args(["-d", "-o"])
            .arg(output)
            .arg(input)
            .status()
            .map_err(|e| e.to_string())?;

        if status.success() {
            Ok(())
        } else {
            Err("age decryption failed".to_string())
        }
    }

    pub(crate) fn protect_file_with_age(&mut self, input: &PathBuf) -> io::Result<()> {
        let protected_path = Self::age_protected_output_path(input);
        if protected_path.exists() {
            self.set_status(format!(
                "protected target exists: {}",
                protected_path.file_name().and_then(|n| n.to_str()).unwrap_or("target")
            ));
            return Ok(());
        }

        suspend_tui()?;
        let result = Self::age_encrypt_file_interactive(input, &protected_path);
        resume_tui_cleared()?;
        enable_raw_mode()?;

        match result {
            Ok(()) => {
                let _ = crate::util::cleanup::safe_cleanup_path(input);
                self.set_status("file protected with age password");
                self.refresh_entries_or_status();
                self.sync_inactive_panel_if_same_dir();
            }
            Err(e) => {
                let _ = crate::util::cleanup::safe_cleanup_path(&protected_path);
                self.set_status(format!("protect failed: {}", e));
            }
        }
        Ok(())
    }

    pub(crate) fn unprotect_file_with_age(&mut self, input: &PathBuf) -> io::Result<()> {
        let plain_path = Self::age_plain_output_path(input);
        if plain_path.exists() {
            self.set_status(format!(
                "unprotect target exists: {}",
                plain_path.file_name().and_then(|n| n.to_str()).unwrap_or("target")
            ));
            return Ok(());
        }

        suspend_tui()?;
        let result = Self::age_decrypt_file_interactive(input, &plain_path);
        resume_tui_cleared()?;
        enable_raw_mode()?;

        match result {
            Ok(()) => {
                let _ = crate::util::cleanup::safe_cleanup_path(input);
                self.set_status("password protection removed");
                self.refresh_entries_or_status();
                self.sync_inactive_panel_if_same_dir();
            }
            Err(e) => {
                let _ = crate::util::cleanup::safe_cleanup_path(&plain_path);
                self.set_status(format!("unprotect failed: {}", e));
            }
        }

        Ok(())
    }

    pub(crate) fn preview_age_file(&mut self, input: &PathBuf) -> io::Result<bool> {
        let Ok((tmp_dir, tmp_path)) = Self::age_temp_decrypt_paths(input, "preview") else {
            self.set_status("failed to prepare temporary file");
            return Ok(false);
        };

        suspend_tui()?;
        let decrypted = Self::age_decrypt_file_interactive(input, &tmp_path);

        let mut shown = false;
        if decrypted.is_ok() {
            if Self::is_image_file(&tmp_path) && self.integration_active("viu") {
                shown = Self::preview_single_image_with_tool(&tmp_path, "viu");
            } else if Self::is_image_file(&tmp_path) && self.integration_active("chafa") {
                shown = Self::preview_single_image_with_tool(&tmp_path, "chafa");
            } else if Self::is_markdown_file(&tmp_path) && self.integration_active("glow") {
                shown = Command::new("glow")
                    .arg("-p")
                    .arg(&tmp_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            } else if Self::is_mermaid_file(&tmp_path) && self.integration_active("mmdflux") {
                let mut cmd = Command::new("mmdflux");
                cmd.arg(&tmp_path);
                shown = crate::util::command::pipe_to_pager(cmd);
            } else if Self::is_html_file(&tmp_path) && self.integration_active("links") {
                shown = Command::new("links")
                    .arg(&tmp_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            } else if Self::is_json_file(&tmp_path) && self.integration_active("jnv") {
                shown = Self::preview_json_with_jnv(&tmp_path)?;
            } else if Self::is_delimited_text_file(&tmp_path) && self.integration_active("csvlens") {
                shown = Command::new("csvlens")
                    .arg(&tmp_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            } else if Self::is_audio_file(&tmp_path) && self.integration_active("sox") {
                let mut child = if Self::integration_probe("play").0 {
                    Command::new("play")
                        .arg(&tmp_path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                } else {
                    Command::new("sox")
                        .arg(&tmp_path)
                        .arg("-d")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                };

                if let Ok(ref mut proc) = child {
                    println!("Playing decrypted audio: {}", input.display());
                    println!("Press q, Esc, or Left to stop playback.");
                    enable_raw_mode()?;
                    loop {
                        if proc.try_wait()?.is_some() {
                            break;
                        }
                        if event::poll(Duration::from_millis(120))?
                            && let Event::Key(k) = event::read()?
                                && matches!(k.code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left) {
                                    let _ = proc.kill();
                                    let _ = proc.wait();
                                    break;
                                }
                    }
                    disable_raw_mode()?;
                    shown = true;
                }
            } else if Self::is_cast_file(&tmp_path) && self.integration_active("asciinema") {
                shown = Self::preview_cast_with_asciinema(&tmp_path)?;
            } else if Self::is_supported_archive(&tmp_path) {
                shown = self.preview_archive_contents(&tmp_path);
            } else if Self::is_pdf_file(&tmp_path) && self.integration_active("pdftotext") {
                let mut cmd = Command::new("pdftotext");
                cmd.args(["-layout", "-nopgbrk"]).arg(&tmp_path).arg("-");
                shown = crate::util::command::pipe_to_pager(cmd);
            } else if Self::is_binary_file(&tmp_path) && self.integration_active("hexyl") {
                let mut cmd = Command::new("hexyl");
                cmd.arg(&tmp_path);
                shown = crate::util::command::pipe_to_pager(cmd);
            } else if self.integration_active("bat") {
                let bat_cmd = Self::bat_tool().unwrap_or_else(|| "bat".to_string());
                shown = Command::new(bat_cmd)
                    .args(["--paging=always", "--style=full", "--color=always"])
                    .arg(&tmp_path)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            } else {
                shown = Command::new("less")
                    .args(["-R", tmp_path.to_str().unwrap_or_default()])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            }
        }

        resume_tui_cleared()?;
        enable_raw_mode()?;
        let _ = fs::remove_file(&tmp_path);
        let _ = fs::remove_dir_all(&tmp_dir);

        if let Err(e) = decrypted {
            self.set_status(format!("decrypt failed: {}", e));
            return Ok(false);
        }
        Ok(shown)
    }

    pub(crate) fn edit_age_file(&mut self, input: &PathBuf) -> io::Result<bool> {
        let Ok((tmp_dir, tmp_path)) = Self::age_temp_decrypt_paths(input, "edit") else {
            self.set_status("failed to prepare temporary file");
            return Ok(false);
        };

        suspend_tui()?;
        execute!(io::stdout(), Show)?;
        let decrypted = Self::age_decrypt_file_interactive(input, &tmp_path);
        if decrypted.is_err() {
            resume_tui()?;
            execute!(io::stdout(), Hide)?;
            let _ = fs::remove_file(&tmp_path);
            let _ = fs::remove_dir_all(&tmp_dir);
            self.set_status(format!("decrypt failed: {}", decrypted.err().unwrap_or_default()));
            return Ok(false);
        }

        let editor = crate::util::command::editor_command();
        let _ = Command::new(editor)
            .arg(&tmp_path)
            .status();

        let result = Self::age_encrypt_file_interactive(&tmp_path, input);
        resume_tui()?;
        execute!(io::stdout(), Hide)?;

        let _ = fs::remove_file(&tmp_path);
        let _ = fs::remove_dir_all(&tmp_dir);
        match result {
            Ok(()) => self.set_status("protected file updated"),
            Err(e) => self.set_status(format!("re-protect failed: {}", e)),
        }
        self.refresh_entries_or_status();
        self.sync_inactive_panel_if_same_dir();
        Ok(true)
    }
}
