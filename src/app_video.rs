//! Live in-pane video playback for kitty-protocol terminals (ghostty, kitty,
//! konsole): timg is spawned with `--pixelation=kitty` on a pty sized like the
//! real terminal, and a relay thread rewrites its output stream so the
//! animation stays anchored inside the preview pane.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::cursor::MoveTo;
use crossterm::execute;
use ratatui::prelude::Rect;

use crate::{App, AppMode};

/// Escape sequences removed from timg's output before it reaches the real
/// terminal: the startup capability queries (XTVERSION, DSR, primary DA) whose
/// replies would otherwise arrive on stdin as stray key events, and the final
/// show-cursor which would unhide the cursor inside the TUI.
const STRIP_SEQUENCES: &[&[u8]] = &[b"\x1b[>q", b"\x1b[5n", b"\x1b[c", b"\x1b[?25h"];

/// Longest APC header we ever need to see in one piece before rewriting it:
/// `ESC _ G <params> ;` plus the 32 base64 chars holding the PNG header.
const APC_HEADER_MAX: usize = 128;

/// Stateful byte filter for timg's kitty-mode output stream. timg sizes and
/// positions frames in pixels derived from the pty winsize, which is only an
/// estimate of the real terminal's cell size — trusting it lets frames spill
/// past the preview pane. The filter therefore rewrites the stream into
/// cell-exact form:
///
/// - every `a=T` kitty transmit is prefixed with an absolute cursor move to
///   the pane origin and gains `c=`/`r=` keys (parsed from the PNG frame
///   dimensions, clamped to the pane) so the terminal itself scales the frame
///   into a cell rectangle that cannot leave the pane;
/// - timg's own between-frames repositioning (`\r`, `\n`, cursor-up), which
///   relies on terminal-specific cursor-after-image behavior, is dropped;
/// - the sequences in [`STRIP_SEQUENCES`] are dropped.
///
/// A small carry buffer keeps sequences split across two reads intact.
pub(crate) struct TimgStreamFilter {
    anchor_row: u16, // 1-based CUP coordinates of the pane origin
    anchor_col: u16,
    max_cols: u16,
    max_rows: u16,
    cell_w: u32, // pixels per cell as advertised to timg via the pty winsize
    cell_h: u32,
    carry: Vec<u8>,
}

impl TimgStreamFilter {
    pub(crate) fn new(area: Rect, cell_w: u32, cell_h: u32) -> Self {
        Self {
            anchor_row: area.y.saturating_add(1),
            anchor_col: area.x.saturating_add(1),
            max_cols: area.width.max(1),
            max_rows: area.height.max(1),
            cell_w: cell_w.max(1),
            cell_h: cell_h.max(1),
            carry: Vec::new(),
        }
    }

    fn emit_anchor(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(format!("\x1b[{};{}H", self.anchor_row, self.anchor_col).as_bytes());
    }

    /// Frame pixel size -> cell rectangle, never larger than the pane.
    fn cell_rect(&self, px_w: u32, px_h: u32) -> (u16, u16) {
        let cols = px_w.div_ceil(self.cell_w).max(1).min(self.max_cols as u32);
        let rows = px_h.div_ceil(self.cell_h).max(1).min(self.max_rows as u32);
        (cols as u16, rows as u16)
    }

    pub(crate) fn process(&mut self, input: &[u8]) -> Vec<u8> {
        let mut data = std::mem::take(&mut self.carry);
        data.extend_from_slice(input);
        let mut out = Vec::with_capacity(data.len() + 32);
        let mut i = 0;
        while i < data.len() {
            let b = data[i];
            if b == b'\r' {
                // timg's per-frame "column 0" reset: re-anchor absolutely so
                // rounding in its own row estimate can never make frames walk.
                self.emit_anchor(&mut out);
                i += 1;
                continue;
            }
            if b == b'\n' {
                // Part of the same repositioning dance; the anchor above
                // already puts the cursor where the next frame goes.
                i += 1;
                continue;
            }
            if b == 0x1b {
                let rest = &data[i..];
                if let Some(seq) = STRIP_SEQUENCES.iter().find(|s| rest.starts_with(s)) {
                    i += seq.len();
                    continue;
                }
                if let Some(len) = match_cursor_up(rest) {
                    i += len; // frame repositioning, superseded by the anchor
                    continue;
                }
                if rest.starts_with(b"\x1b_G") {
                    match self.rewrite_apc_header(rest, &mut out) {
                        Some(consumed) => {
                            i += consumed;
                            continue;
                        }
                        None => {
                            // Header not complete in this read yet.
                            self.carry = rest.to_vec();
                            break;
                        }
                    }
                }
                // Any of the sequences above may be cut off at the end of this
                // read: hold the tail back until the next chunk completes it.
                if is_possible_sequence_prefix(rest) {
                    self.carry = rest.to_vec();
                    break;
                }
            }
            out.push(b);
            i += 1;
        }
        out
    }

    /// Emit the kitty APC header starting at `data` (which begins with
    /// `ESC _ G`), adding `c=`/`r=` to `a=T` transmits. Returns the number of
    /// input bytes consumed (payload bytes stream through the normal loop), or
    /// `None` when the header is still incomplete.
    fn rewrite_apc_header(&self, data: &[u8], out: &mut Vec<u8>) -> Option<usize> {
        let body = &data[3..];
        let semi = match body.iter().position(|&b| b == b';') {
            Some(p) => p,
            None => {
                if body.contains(&0x1b) || body.len() > APC_HEADER_MAX {
                    // Payload-less command (e.g. a delete): pass through as-is.
                    out.extend_from_slice(b"\x1b_G");
                    return Some(3);
                }
                return None; // ';' may still arrive in the next read
            }
        };
        let params = &body[..semi];
        let is_png_transmit = params
            .split(|&b| b == b',')
            .any(|kv| kv == b"a=T")
            && params.split(|&b| b == b',').any(|kv| kv == b"f=100");
        if !is_png_transmit {
            out.extend_from_slice(b"\x1b_G");
            return Some(3);
        }
        // The first 32 base64 chars decode to the 24 bytes covering the PNG
        // signature and the IHDR width/height.
        let payload = &body[semi + 1..];
        if payload.len() < 32 {
            if payload.contains(&0x1b) {
                // Malformed/truncated transmit: pass through untouched.
                out.extend_from_slice(b"\x1b_G");
                return Some(3);
            }
            return None;
        }
        use base64::Engine as _;
        let head = base64::engine::general_purpose::STANDARD.decode(&payload[..32]);
        let dims = head.ok().filter(|h| h.starts_with(b"\x89PNG\r\n\x1a\n")).map(|h| {
            (
                u32::from_be_bytes([h[16], h[17], h[18], h[19]]),
                u32::from_be_bytes([h[20], h[21], h[22], h[23]]),
            )
        });
        // Every frame is positioned absolutely: timg's own repositioning
        // (LF + cursor-up, relying on terminal-specific cursor-after-image
        // behavior) is dropped by the main loop and cannot be trusted.
        self.emit_anchor(out);
        out.extend_from_slice(b"\x1b_G");
        out.extend_from_slice(params);
        if let Some((w, h)) = dims {
            let (cols, rows) = self.cell_rect(w, h);
            out.extend_from_slice(format!(",c={cols},r={rows}").as_bytes());
        }
        out.push(b';');
        Some(3 + semi + 1)
    }

    pub(crate) fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.carry)
    }
}

/// Length of a CSI cursor-up sequence (`ESC [ <digits> A`) at the start of
/// `data`, if one is there.
fn match_cursor_up(data: &[u8]) -> Option<usize> {
    if !data.starts_with(b"\x1b[") {
        return None;
    }
    let digits = data[2..].iter().take_while(|b| b.is_ascii_digit()).count();
    (data.get(2 + digits) == Some(&b'A')).then_some(2 + digits + 1)
}

/// True when `data` (starting with ESC, at the end of a read) could still grow
/// into a sequence the filter needs to see whole: a strip sequence, a
/// cursor-up, or a kitty APC introducer.
fn is_possible_sequence_prefix(data: &[u8]) -> bool {
    if STRIP_SEQUENCES
        .iter()
        .any(|s| s.len() > data.len() && s.starts_with(data))
    {
        return true;
    }
    if b"\x1b_G".starts_with(data) {
        return true;
    }
    // `ESC [ <digits...>` may still become a cursor-up.
    data.starts_with(b"\x1b[") && data[2..].iter().all(|b| b.is_ascii_digit())
}

/// A running (or ended) timg playback bound to one video path and pane area.
/// Kept around after playback stops so the same video is not restarted until
/// the selection or layout changes.
pub(crate) struct PaneVideoSession {
    pub(crate) path: PathBuf,
    pub(crate) area: Rect,
    child: Child,
    relay: Option<std::thread::JoinHandle<()>>,
    playing: bool,
}

impl PaneVideoSession {
    /// True while the timg child is still producing frames. Reaps the child
    /// and joins the relay thread once it has exited on its own.
    pub(crate) fn is_playing(&mut self) -> bool {
        if self.playing && matches!(self.child.try_wait(), Ok(Some(_))) {
            self.playing = false;
            self.join_relay();
        }
        self.playing
    }

    /// Kill the child and wait for the relay to drain. The last drawn frame is
    /// intentionally left on screen (acts like pausing); image cleanup happens
    /// in [`App::discard_pane_video`].
    pub(crate) fn stop_playback(&mut self) {
        if self.playing {
            let _ = self.child.kill();
            self.playing = false;
        }
        let _ = self.child.wait();
        self.join_relay();
    }

    fn join_relay(&mut self) {
        if let Some(handle) = self.relay.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PaneVideoSession {
    fn drop(&mut self) {
        self.stop_playback();
    }
}

impl App {
    /// Spawn timg playing `path` with kitty graphics into `area`. The child
    /// writes to a pty carrying the real terminal's winsize (timg needs
    /// ws_xpixel/ws_ypixel from TIOCGWINSZ to animate with kitty graphics);
    /// a relay thread forwards the filtered stream to the terminal.
    #[cfg(unix)]
    pub(crate) fn start_pane_video(path: PathBuf, area: Rect) -> Option<PaneVideoSession> {
        use std::os::unix::io::FromRawFd;

        let (cols, rows, xpixel, ypixel) = match crossterm::terminal::window_size() {
            Ok(ws) if ws.columns > 0 && ws.rows > 0 => {
                let xp = if ws.width > 0 { ws.width } else { ws.columns.saturating_mul(8) };
                let yp = if ws.height > 0 { ws.height } else { ws.rows.saturating_mul(16) };
                (ws.columns, ws.rows, xp, yp)
            }
            _ => (80, 24, 640, 384),
        };

        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: xpixel,
            ws_ypixel: ypixel,
        };
        let ok = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &winsize,
            ) == 0
        };
        if !ok {
            return None;
        }
        // Raw pty output so the line discipline doesn't rewrite timg's bytes
        // (ONLCR would inject extra carriage returns).
        unsafe {
            let mut term: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(slave, &mut term) == 0 {
                libc::cfmakeraw(&mut term);
                let _ = libc::tcsetattr(slave, libc::TCSANOW, &term);
            }
        }

        let child = Command::new("timg")
            .arg(format!("-g{}x{}", area.width.max(4), area.height.max(2)))
            .args(["-pk", "--loops=1"])
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(unsafe { Stdio::from_raw_fd(slave) })
            .stderr(Stdio::null())
            .spawn();
        let child = match child {
            Ok(c) => c,
            Err(_) => {
                unsafe { libc::close(master) };
                return None;
            }
        };

        // Anchor the first frame at the pane origin. Draws are suppressed
        // while the session is playing, so nothing moves the cursor between
        // here and the frames that follow.
        {
            let mut out = io::stdout();
            let _ = execute!(out, MoveTo(area.x, area.y));
            let _ = out.flush();
        }

        let mut master_file = unsafe { std::fs::File::from_raw_fd(master) };
        // The same per-cell pixel estimate timg derives from the pty winsize;
        // the filter uses it to convert frame pixel sizes back into cells.
        let (cell_w, cell_h) = ((xpixel / cols) as u32, (ypixel / rows) as u32);
        let relay = std::thread::spawn(move || {
            let mut filter = TimgStreamFilter::new(area, cell_w, cell_h);
            let mut buf = [0u8; 8192];
            let mut out = io::stdout();
            loop {
                match master_file.read(&mut buf) {
                    Ok(0) | Err(_) => break, // EOF/EIO once the child is gone
                    Ok(n) => {
                        let bytes = filter.process(&buf[..n]);
                        if !bytes.is_empty() {
                            let _ = out.write_all(&bytes);
                            let _ = out.flush();
                        }
                    }
                }
            }
            let _ = out.write_all(&filter.finish());
            // If the child was killed mid-frame the terminal may still be
            // consuming an unterminated kitty APC command; a lone string
            // terminator is ignored otherwise.
            let _ = out.write_all(b"\x1b\\");
            let _ = out.flush();
        });

        Some(PaneVideoSession {
            path,
            area,
            child,
            relay: Some(relay),
            playing: true,
        })
    }

    #[cfg(not(unix))]
    pub(crate) fn start_pane_video(_path: PathBuf, _area: Rect) -> Option<PaneVideoSession> {
        None
    }

    pub(crate) fn pane_video_is_playing(&mut self) -> bool {
        self.pane_video.as_mut().map(|s| s.is_playing()).unwrap_or(false)
    }

    /// Any user input while a video is playing stops the playback (leaving the
    /// current frame visible) before the key/mouse event is handled normally.
    pub(crate) fn pause_pane_video_for_input(&mut self) {
        if let Some(session) = &mut self.pane_video {
            if session.is_playing() {
                session.stop_playback();
                self.needs_redraw = true;
            }
        }
    }

    /// Tear the session down completely: kill the child and delete every
    /// visible kitty image placement (timg picks its own image ids, so a
    /// targeted delete is not possible). The still-image dedup key is reset
    /// because the preview image is deleted along with the video frames.
    pub(crate) fn discard_pane_video(&mut self) {
        if let Some(mut session) = self.pane_video.take() {
            session.stop_playback();
            let mut out = io::stdout();
            let _ = write!(out, "\x1b_Ga=d,d=A,q=2\x1b\\");
            let _ = out.flush();
            self.preview.native_last_key = None;
            self.needs_redraw = true;
        }
    }

    /// Called once per event-loop iteration: starts playback when a video is
    /// selected in preview mode on a kitty terminal (after a short debounce so
    /// scrolling through a directory doesn't spawn a timg per row), and stops
    /// it when the selection, layout, or mode moves away.
    pub(crate) fn sync_pane_video(&mut self) {
        use crate::integration::probe::TerminalImageProtocol;

        let target = self
            .preview
            .target_path
            .clone()
            .filter(|p| Self::is_video_file(p));
        let area = self.preview.native_area;
        let want = matches!(
            Self::terminal_image_protocol().0,
            TerminalImageProtocol::Kitty
        ) && self.is_preview_mode()
            && self.mode == AppMode::Browsing
            && !self.preview.pending
            && target.is_some()
            && area.is_some()
            && self.integration_active("timg");

        if let Some(session) = &mut self.pane_video {
            if want && Some(&session.path) == target.as_ref() && Some(session.area) == area {
                // Same video, same pane: keep it whether it is still playing,
                // finished, or was paused by a keypress (no auto-restart).
                let _ = session.is_playing();
                return;
            }
            self.discard_pane_video();
        }

        if !want {
            self.pane_video_want_since = None;
            return;
        }
        let (path, area) = (target.unwrap(), area.unwrap());
        match &self.pane_video_want_since {
            Some((p, since)) if *p == path => {
                if since.elapsed() >= Duration::from_millis(300) {
                    self.pane_video = Self::start_pane_video(path, area);
                    self.pane_video_want_since = None;
                }
            }
            _ => self.pane_video_want_since = Some((path, Instant::now())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TimgStreamFilter;
    use base64::Engine as _;
    use ratatui::prelude::Rect;

    fn filter() -> TimgStreamFilter {
        // Pane at (45, 2), 87x32 cells; 10x21 px cells.
        TimgStreamFilter::new(Rect::new(45, 2, 87, 32), 10, 21)
    }

    /// Base64 of a PNG header for a `w` x `h` image (signature + IHDR dims).
    fn png_head_b64(w: u32, h: u32) -> String {
        let mut head = Vec::new();
        head.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        head.extend_from_slice(&13u32.to_be_bytes());
        head.extend_from_slice(b"IHDR");
        head.extend_from_slice(&w.to_be_bytes());
        head.extend_from_slice(&h.to_be_bytes());
        base64::engine::general_purpose::STANDARD.encode(head)
    }

    #[test]
    fn test_timg_filter_replaces_frame_repositioning_with_anchor() {
        let mut f = filter();
        let out = f.process(b"abc\r\n\x1b[24A");
        // CR -> absolute pane-origin CUP; LF and cursor-up dropped.
        assert_eq!(out, b"abc\x1b[3;46H");
    }

    #[test]
    fn test_timg_filter_strips_queries_and_show_cursor() {
        let mut f = filter();
        let out = f.process(b"\x1b[>q\x1b[5n\x1b[c\x1b[?25ldata\x1b[?25h");
        assert_eq!(out, b"\x1b[?25ldata");
    }

    #[test]
    fn test_timg_filter_handles_sequence_split_across_chunks() {
        let mut f = filter();
        let mut out = f.process(b"x\x1b[?2");
        out.extend(f.process(b"5hy\x1b[2"));
        out.extend(f.process(b"4Az"));
        out.extend(f.finish());
        assert_eq!(out, b"xyz");
    }

    #[test]
    fn test_timg_filter_adds_cell_rect_to_png_transmit() {
        let mut f = filter();
        // 870x489 px at 10x21 px cells -> 87 cols, ceil(489/21) = 24 rows.
        let input = format!("\x1b_Ga=T,i=7,q=2,f=100,m=1;{}\x1b\\", png_head_b64(870, 489));
        let out = f.process(input.as_bytes());
        let expected = format!(
            "\x1b[3;46H\x1b_Ga=T,i=7,q=2,f=100,m=1,c=87,r=24;{}\x1b\\",
            png_head_b64(870, 489)
        );
        assert_eq!(out, expected.as_bytes());
        assert!(f.finish().is_empty());
    }

    #[test]
    fn test_timg_filter_clamps_cell_rect_to_pane() {
        let mut f = filter();
        // A frame reported far larger than the pane still maps to <= 87x32.
        let input = format!("\x1b_Ga=T,q=2,f=100,m=1;{}", png_head_b64(2000, 2000));
        let out = f.process(input.as_bytes());
        let expected = format!("\x1b[3;46H\x1b_Ga=T,q=2,f=100,m=1,c=87,r=32;{}", png_head_b64(2000, 2000));
        assert_eq!(out, expected.as_bytes());
    }

    #[test]
    fn test_timg_filter_apc_split_across_chunks() {
        let mut f = filter();
        let b64 = png_head_b64(870, 489);
        let (head, tail) = b64.split_at(10);
        let mut out = f.process(format!("\x1b_Ga=T,q=2,f=100,m=1;{head}").as_bytes());
        out.extend(f.process(tail.as_bytes()));
        out.extend(f.finish());
        let expected = format!("\x1b[3;46H\x1b_Ga=T,q=2,f=100,m=1,c=87,r=24;{b64}");
        assert_eq!(out, expected.as_bytes());
    }

    #[test]
    fn test_timg_filter_passes_continuation_chunks_through() {
        let mut f = filter();
        let input: &[u8] = b"\x1b_Gq=2,m=0;AAAA\x1b\\";
        let out = f.process(input);
        assert_eq!(out, input);
        assert!(f.finish().is_empty());
    }
}
