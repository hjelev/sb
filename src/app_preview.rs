use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType},
};
use crate::{App, PreviewContentMsg, PreviewPaneFocus};

impl App {
    pub(crate) fn toggle_preview_mode(&mut self) {
        self.preview_enabled = !self.preview_enabled;
        self.preview_scroll_offset = 0;
        if self.preview_enabled {
            self.preview_pane_focus = PreviewPaneFocus::Folder;
            self.preview_lines = vec!["Loading preview...".to_string()];
            self.preview_footer = None;
            self.preview_image_rgb = None;
            self.preview_image_png = None;
            self.preview_native_last_key = None;
            self.request_preview_for_selected();
        } else {
            if self.preview_native_last_key.is_some() {
                match Self::terminal_image_protocol().0 {
                    crate::integration::probe::TerminalImageProtocol::Kitty => {
                        let _ = Self::clear_kitty_pane_images();
                    }
                    crate::integration::probe::TerminalImageProtocol::Iterm2Inline
                    | crate::integration::probe::TerminalImageProtocol::Sixel => {
                        if let Some(area) = self.preview_native_area {
                            let _ = Self::clear_preview_pane_area(
                                area.x,
                                area.y,
                                area.width,
                                area.height,
                            );
                        }
                    }
                    _ => {}
                }
            }
            self.preview_target_path = None;
            self.preview_lines.clear();
            self.preview_footer = None;
            self.preview_pending = false;
            self.preview_rx = None;
            self.preview_native_area = None;
            self.preview_native_last_key = None;
            self.preview_image_rgb = None;
            self.preview_image_png = None;
            self.preview_pane_focus = PreviewPaneFocus::Folder;
        }
    }

    pub(crate) fn toggle_preview_pane_focus(&mut self) {
        self.preview_pane_focus = match self.preview_pane_focus {
            PreviewPaneFocus::Folder => PreviewPaneFocus::Preview,
            PreviewPaneFocus::Preview => PreviewPaneFocus::Folder,
        };
    }

    pub(crate) fn preview_focus_is_preview(&self) -> bool {
        self.preview_enabled && self.preview_pane_focus == PreviewPaneFocus::Preview
    }

    pub(crate) fn preview_max_scroll(&self) -> usize {
        self.preview_lines.len().saturating_sub(1)
    }

    pub(crate) fn preview_scroll_up(&mut self, amount: usize) {
        self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(amount);
    }

    pub(crate) fn preview_scroll_down(&mut self, amount: usize) {
        let next = self.preview_scroll_offset.saturating_add(amount);
        self.preview_scroll_offset = next.min(self.preview_max_scroll());
    }

    pub(crate) fn request_preview_for_selected(&mut self) {
        if !self.preview_enabled {
            return;
        }
        let Some(path) = self.entries.get(self.selected_index).map(|e| e.path()) else {
            self.preview_lines = vec!["No selection".to_string()];
            self.preview_footer = None;
            self.preview_target_path = None;
            self.preview_pending = false;
            self.preview_rx = None;
            self.preview_image_rgb = None;
            self.preview_image_png = None;
            return;
        };

        if self.preview_target_path.as_ref() == Some(&path)
            && (self.preview_pending
                || !self.preview_lines.is_empty()
                || self.preview_image_rgb.is_some())
        {
            return;
        }

        // Path changed: clear stale image data.
        self.preview_image_rgb = None;
        self.preview_image_png = None;

        // Image files skip text cache (their render path uses decoded RGB).
        if !Self::is_image_file(&path) {
            if let Some(cached) = self.preview_cache.get(&path).cloned() {
                self.preview_target_path = Some(path);
                self.preview_lines = cached.0;
                self.preview_footer = cached.1;
                self.preview_pending = false;
                self.preview_scroll_offset = 0;
                return;
            }
        }

        self.preview_request_id = self.preview_request_id.saturating_add(1);
        let request_id = self.preview_request_id;
        self.preview_target_path = Some(path.clone());
        self.preview_pending = true;
        self.preview_scroll_offset = 0;
        self.preview_lines = vec!["Loading preview...".to_string()];
        self.preview_footer = None;

        let use_bat = Self::integration_availability_and_detail("bat").0;
        let use_file = Self::integration_availability_and_detail("file").0;
        let use_resvg = self.integration_active("resvg");
        let show_icons = self.show_icons;
        let nerd_font_active = self.nerd_font_active;
        let (tx, rx) = mpsc::channel();
        self.preview_rx = Some(rx);
        thread::spawn(move || {
            let msg = App::build_preview_content(
                request_id,
                path,
                use_bat,
                use_file,
                use_resvg,
                show_icons,
                nerd_font_active,
            );
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn pump_preview_progress(&mut self) {
        let Some(rx) = self.preview_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                footer,
                image_rgb,
            }) => {
                if request_id == self.preview_request_id {
                    self.preview_target_path = Some(path.clone());
                    if image_rgb.is_none() {
                        self.preview_cache
                            .insert(path.clone(), (lines.clone(), footer.clone()));
                    }
                    self.preview_lines = lines;
                    self.preview_footer = footer;
                    if let Some((ref rgb, w, h)) = image_rgb {
                        self.preview_image_png = App::encode_rgb_to_png(rgb, w, h);
                    } else {
                        self.preview_image_png = None;
                    }
                    self.preview_image_rgb = image_rgb;
                    self.preview_pending = false;
                    self.preview_scroll_offset = 0;
                }
            }
            Ok(PreviewContentMsg::Failed {
                request_id,
                path,
                message,
            }) => {
                if request_id == self.preview_request_id {
                    self.preview_target_path = Some(path);
                    self.preview_lines = vec![message];
                    self.preview_footer = None;
                    self.preview_image_rgb = None;
                    self.preview_image_png = None;
                    self.preview_pending = false;
                    self.preview_scroll_offset = 0;
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.preview_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.preview_pending = false;
            }
        }
    }

    pub(crate) fn build_preview_content(
        request_id: u64,
        path: PathBuf,
        use_bat: bool,
        use_file: bool,
        use_resvg: bool,
        show_icons: bool,
        nerd_font_active: bool,
    ) -> PreviewContentMsg {
        if path.is_dir() {
            let mut entries = Vec::new();
            let mut names = Vec::new();
            if let Ok(read_dir) = fs::read_dir(&path) {
                for item in read_dir.flatten().take(500) {
                    names.push(item.path());
                }
            }
            names.sort_by(|a, b| {
                let a_name = a
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let b_name = b
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                a_name.cmp(&b_name)
            });

            for entry_path in names {
                let file_name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| entry_path.to_string_lossy().into_owned());
                let is_symlink = entry_path
                    .symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                let is_dir = entry_path.is_dir();
                let (icon_glyph, _) = App::icon_for_name(
                    &file_name,
                    is_dir,
                    show_icons,
                    nerd_font_active,
                    is_symlink,
                );
                let icon_prefix = if show_icons && !icon_glyph.is_empty() {
                    format!("{} ", icon_glyph)
                } else {
                    String::new()
                };
                let suffix = if is_dir { "/" } else { "" };
                entries.push(format!("{}{}{}", icon_prefix, file_name, suffix));
            }

            if entries.is_empty() {
                entries.push("[empty folder]".to_string());
            }

            let footer = App::compute_total_display_bytes(&path)
                .ok()
                .map(|bytes| format!("Total: {}", App::format_size(bytes)));
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines: entries,
                footer,
                image_rgb: None,
            };
        }

        if !path.exists() {
            return PreviewContentMsg::Failed {
                request_id,
                path,
                message: "[file not found]".to_string(),
            };
        }

        if App::is_svg_file(&path) && use_resvg {
            let image_rgb = App::decode_svg_to_rgb_scaled(&path);
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let lines = if image_rgb.is_none() {
                vec!["[svg could not be rendered]".to_string()]
            } else {
                Vec::new()
            };
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                footer,
                image_rgb,
            };
        }

        if App::is_image_file(&path) {
            let image_rgb = App::decode_image_to_rgb_scaled(&path);
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            let lines = if image_rgb.is_none() {
                vec!["[image could not be decoded]".to_string()]
            } else {
                Vec::new()
            };
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                footer,
                image_rgb,
            };
        }

        let mut lines: Vec<String> = Vec::new();

        if App::is_binary_file(&path) {
            lines.push("[binary file]".to_string());
            if use_file {
                if let Ok(out) = Command::new("file").arg("-b").arg(&path).output() {
                    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !text.is_empty() {
                        lines.push(text);
                    }
                }
            }
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let footer = Some(format!("Size: {}", App::format_size(size)));
            return PreviewContentMsg::Ready {
                request_id,
                path,
                lines,
                footer,
                image_rgb: None,
            };
        }

        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > 10 * 1024 * 1024 {
            lines.push("[preview truncated: file larger than 10MB]".to_string());
        }

        if use_bat {
            if let Ok(out) = Command::new("bat")
                .args([
                    "--paging=never",
                    "--style=plain",
                    "--color=always",
                    "--line-range",
                    "1:220",
                ])
                .arg(&path)
                .output()
            {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).into_owned();
                    lines.extend(text.lines().take(220).map(|s| s.to_string()));
                }
            }
        }

        if lines.is_empty() {
            let mut bytes = Vec::new();
            if let Ok(mut file) = fs::File::open(&path) {
                let _ = file.read_to_end(&mut bytes);
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            lines.extend(text.lines().take(220).map(|s| s.to_string()));
        }

        if lines.is_empty() {
            lines.push("[no preview output]".to_string());
        }

        let footer = Some(format!("Size: {}", App::format_size(size)));
        PreviewContentMsg::Ready {
            request_id,
            path,
            lines,
            footer,
            image_rgb: None,
        }
    }

    pub(crate) fn preview_json_with_jnv(path: &PathBuf) -> io::Result<bool> {
        let mut child = Command::new("jnv").arg(path).spawn();
        if let Ok(ref mut proc) = child {
            println!("Viewing JSON: {}", path.display());
            println!("Press q, Esc, or Left to close preview.");
            enable_raw_mode()?;
            loop {
                if proc.try_wait()?.is_some() {
                    break;
                }
                if event::poll(Duration::from_millis(120))? {
                    if let Event::Key(k) = event::read()? {
                        if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left) {
                            let _ = proc.kill();
                            let _ = proc.wait();
                            break;
                        }
                    }
                }
            }
            disable_raw_mode()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn preview_single_image_with_tool(path: &PathBuf, tool: &str) -> bool {
        let script = r#"
tool="$1"
img="$2"
clear
"$tool" -- "$img"
printf '\n[Press any key to return]\n'
IFS= read -rsn1 _
"#;

        Command::new("bash")
            .arg("-lc")
            .arg(script)
            .arg("--")
            .arg(tool)
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub(crate) fn preview_cast_with_asciinema(path: &PathBuf) -> io::Result<bool> {
        execute!(io::stdout(), TermClear(ClearType::All), crossterm::cursor::MoveTo(0, 0))?;

        let mut child = match Command::new("asciinema")
            .arg("play")
            .arg(path)
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        println!("Playing cast: {}", path.display());
        println!("Press q or Esc to stop playback.");

        enable_raw_mode()?;
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if event::poll(Duration::from_millis(120))? {
                if let Event::Key(k) = event::read()? {
                    if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
        disable_raw_mode()?;
        Ok(true)
    }
}
