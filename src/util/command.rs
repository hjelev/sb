//! Command execution builder pattern.
//!
//! Replaces scattered `Command::new()` patterns with a builder that:
//! - Captures stderr for better error messages
//! - Returns consistent Result types (no silent failures)
//! - Provides high-level methods for common commands (git, archive, preview)

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Spawn `cmd` with its stdout piped into `less -R`, returning whether the pager
/// exited successfully. The spawned child is reaped after the pager closes.
///
/// Centralizes the repeated "spawn preview tool → pipe into `less -R`" blocks
/// (mermaid/`mmdflux`, `pdftotext`, `hexyl`, …). Callers that don't care about
/// success use `let _ = pipe_to_pager(cmd);`.
///
/// The producer runs detached from the tty ([`detach_from_tty`]): tools like
/// `mmdflux` otherwise open `/dev/tty`, flip it into raw mode for a terminal
/// query, and later restore the termios *they* saw at startup — clobbering the
/// pager's cbreak state so its keys (`q`) sit unread in a canonical buffer.
///
/// The pager runs via [`status_ignoring_sigint`]: raw mode is suspended here,
/// so a Ctrl+C must kill only the children, not sb. A producer still alive
/// after the pager closes is killed rather than waited on — nothing will read
/// its output anymore, and a tool blocked on e.g. a terminal query it will
/// never get an answer to (the pager consumed it) would wedge sb forever.
pub fn pipe_to_pager(mut cmd: Command) -> bool {
    detach_from_tty(&mut cmd);
    let Ok(mut child) = cmd.stdout(Stdio::piped()).spawn() else {
        return false;
    };
    let mut shown = false;
    if let Some(out) = child.stdout.take() {
        let mut pager = Command::new("less");
        pager.args(["-R"]).stdin(out);
        shown = status_ignoring_sigint(&mut pager)
            .map(|s| s.success())
            .unwrap_or(false);
    }
    let _ = child.kill();
    let _ = child.wait();
    shown
}

/// Spawn `git <args>` in `cwd` with its stdout piped into `tool`'s stdin,
/// leaving the terminal otherwise inherited so stdin-fed diff pagers like
/// `diffnav` can take over the screen. Returns whether the pager ran and
/// exited successfully, so callers can fall back to another diff viewer.
pub fn pipe_git_to_tool<P: AsRef<Path>>(
    cwd: P,
    git_args: &[&str],
    tool: &str,
    tool_args: &[&str],
) -> bool {
    let Ok(mut git) = Command::new("git")
        .args(git_args)
        .current_dir(cwd.as_ref())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    else {
        return false;
    };
    let mut shown = false;
    if let Some(out) = git.stdout.take() {
        shown = Command::new(tool)
            .args(tool_args)
            .stdin(out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    let _ = git.wait();
    shown
}

/// Page `cmd`'s output through `less -R`, falling back to paging `path` directly
/// when the tool can't be spawned.
///
/// Centralizes the "run viewer → pipe into less → otherwise just `less` the
/// file" idiom repeated by the hexyl / pdftotext viewers.
pub fn pipe_to_pager_or_less(cmd: Command, path: &Path) {
    if !pipe_to_pager(cmd) {
        let _ = Command::new("less").arg("-R").arg(path).status();
    }
}

/// Make `cmd` run in its own session with no controlling terminal (`setsid`
/// in the child), so tools that open `/dev/tty` for terminal-appearance
/// queries or termios tweaks fail fast (`ENXIO`) instead of blocking on a
/// reply another tty reader will consume, or corrupting that reader's
/// terminal state. Use for non-interactive producers whose output is
/// captured or piped; never for tools that must own the terminal.
pub fn detach_from_tty(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
}

/// Signal dispositions are process-wide, so overlapping callers (e.g. from
/// different threads) must not each save/restore independently: a caller
/// restoring `SIG_DFL` while another child is still running would leave sb
/// killable by that child's Ctrl+C. The first acquirer saves the previous
/// handler and installs `SIG_IGN`; the last one to drop restores it.
#[cfg(unix)]
struct SigintIgnoreGuard;

#[cfg(unix)]
static SIGINT_IGNORE: std::sync::Mutex<(usize, libc::sighandler_t)> =
    std::sync::Mutex::new((0, libc::SIG_DFL));

#[cfg(unix)]
impl SigintIgnoreGuard {
    fn acquire() -> Self {
        let mut state = SIGINT_IGNORE.lock().unwrap_or_else(|e| e.into_inner());
        if state.0 == 0 {
            state.1 = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        }
        state.0 += 1;
        SigintIgnoreGuard
    }
}

#[cfg(unix)]
impl Drop for SigintIgnoreGuard {
    fn drop(&mut self) {
        let mut state = SIGINT_IGNORE.lock().unwrap_or_else(|e| e.into_inner());
        state.0 -= 1;
        if state.0 == 0 {
            unsafe {
                libc::signal(libc::SIGINT, state.1);
            }
        }
    }
}

/// Run `cmd` to completion while this process ignores SIGINT, so a Ctrl+C
/// aimed at a full-screen child (image slideshow, video playback) kills only
/// the child and not sb, whose raw mode is suspended while the child runs.
/// The child is reset to the default SIGINT disposition via `pre_exec` — an
/// ignored disposition is inherited across exec and would otherwise make the
/// child itself immune to Ctrl+C.
pub fn status_ignoring_sigint(cmd: &mut Command) -> io::Result<std::process::ExitStatus> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                Ok(())
            });
        }
        let _guard = SigintIgnoreGuard::acquire();
        cmd.status()
    }
    #[cfg(not(unix))]
    {
        cmd.status()
    }
}

/// Run `cmd` detached from the tty with stdout/stderr captured, killing it
/// (and everything it spawned) if it hasn't exited within `timeout`.
///
/// The child leads its own session ([`detach_from_tty`]), so on timeout the
/// whole process group is killed — otherwise a `sh -c` grandchild would keep
/// the pipes open and the reader threads would never finish.
pub fn output_with_timeout(mut cmd: Command, timeout: Duration) -> io::Result<Output> {
    detach_from_tty(&mut cmd);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let read_all = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let stdout = read_all(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = read_all(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    match status {
        Some(status) => Ok(Output {
            status,
            stdout,
            stderr,
        }),
        None => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("timed out after {}s", timeout.as_secs()),
        )),
    }
}

/// Spawn `program path [args…]` fully detached (all stdio set to null) and
/// return the [`Child`] so the caller can wait on or kill it.
///
/// Centralizes the duplicated null-stdio spawn used by the audio players
/// (`play` / `sox`).
pub fn spawn_detached(program: &str, path: &Path, args: &[&str]) -> io::Result<Child> {
    Command::new(program)
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// The user's preferred terminal editor program.
///
/// Reads `$EDITOR`, falling back to `nano`. Centralizes the
/// `env::var("EDITOR").unwrap_or_else(|_| "nano".to_string())` pattern that was
/// duplicated across the editor/notes/transfer launch sites.
pub fn editor_command() -> String {
    std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string())
}

/// A `Command` that launches `$EDITOR`. Like git, the variable may carry
/// arguments (`EDITOR="code -w"`, `EDITOR="nvim -u NONE"`); a bare
/// `Command::new("code -w")` would look for a binary literally named that.
pub fn editor() -> Command {
    let mut words = editor_argv().into_iter();
    let mut cmd = Command::new(words.next().unwrap_or_else(|| "nano".to_string()));
    cmd.args(words);
    cmd
}

/// Program name of `$EDITOR` (its first word), for PATH probing.
pub fn editor_program() -> String {
    editor_argv()
        .into_iter()
        .next()
        .unwrap_or_else(|| "nano".to_string())
}

fn editor_argv() -> Vec<String> {
    let raw = editor_command();
    split_shell_words(&raw).unwrap_or_else(|| vec![raw])
}

/// Minimal POSIX-ish word splitting: whitespace separates words, single
/// quotes are literal, double quotes allow `\"`/`\\` escapes, and a bare
/// backslash escapes the next char. Returns `None` on an unterminated quote.
fn split_shell_words(s: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => cur.push(c),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            c @ ('"' | '\\' | '$' | '`') => cur.push(c),
                            c => {
                                cur.push('\\');
                                cur.push(c);
                            }
                        },
                        c => cur.push(c),
                    }
                }
            }
            '\\' => {
                in_word = true;
                cur.push(chars.next()?);
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Some(words)
}

/// Run an interactive program (viewer, pager, editor) to completion on the
/// inherited terminal.
///
/// The program's own stderr is wiped when the TUI resumes, so failures are
/// returned as a short message for the status line instead: missing binary,
/// spawn error, or non-zero exit.
///
/// Runs via [`status_ignoring_sigint`]: raw mode is off while the program
/// owns the terminal, so a Ctrl+C (e.g. in `less` or `glow -p`) would
/// otherwise also reach sb and kill it without unmounting or restoring state.
pub fn run_interactive(cmd: &mut Command) -> Result<(), String> {
    let tool = program_name(cmd);
    let via_path = searched_in_path(cmd);
    describe_exit(&tool, via_path, status_ignoring_sigint(cmd))
}

/// Run `cmd` on the inherited terminal for the non-TUI CLI modes (`sb FILE`,
/// `sb -l FILE`, `sb -e FILE`) and return a shell-style exit code for sb to
/// exit with.
/// Launch failures are reported on stderr the way shells do: 127 when the
/// program isn't found, 126 for other spawn errors.
pub fn run_cli(cmd: &mut Command) -> i32 {
    let tool = program_name(cmd);
    let via_path = searched_in_path(cmd);
    cli_exit_code(&tool, via_path, cmd.status())
}

fn cli_exit_code(tool: &str, via_path: bool, status: io::Result<std::process::ExitStatus>) -> i32 {
    match status {
        Ok(s) => exit_status_code(s),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if via_path {
                eprintln!("sb: {tool}: command not found");
            } else {
                eprintln!("sb: {tool}: No such file or directory");
            }
            127
        }
        Err(e) => {
            eprintln!("sb: {tool}: {e}");
            126
        }
    }
}

/// The exit code a shell would report: the code itself, or 128+signal.
fn exit_status_code(status: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    status.code().unwrap_or(1)
}

fn program_name(cmd: &Command) -> String {
    Path::new(cmd.get_program())
        .file_name()
        .unwrap_or(cmd.get_program())
        .to_string_lossy()
        .into_owned()
}

/// Whether the OS will look the program up in `PATH` (no `/` in its name),
/// so "not found" can say where it looked.
fn searched_in_path(cmd: &Command) -> bool {
    !cmd.get_program().to_string_lossy().contains('/')
}

fn describe_exit(
    tool: &str,
    via_path: bool,
    status: io::Result<std::process::ExitStatus>,
) -> Result<(), String> {
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(match s.code() {
            Some(code) => format!("{tool} exited with status {code}"),
            None => format!("{tool} was terminated by a signal"),
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(if via_path {
            format!("{tool} not found in PATH")
        } else {
            format!("{tool} not found")
        }),
        Err(e) => Err(format!("{tool} failed to start: {e}")),
    }
}

/// Builder for executing external commands with consistent error handling.
pub struct CommandBuilder;

impl CommandBuilder {
    /// Run a git command in the given directory.
    ///
    /// # Arguments
    /// * `cwd` - Working directory for the command
    /// * `args` - Git arguments (e.g., ["status", "--porcelain"])
    ///
    /// # Returns
    /// * `Ok(Output)` with stdout/stderr captured
    /// * `Err` if git command fails
    ///
    /// # Example
    /// ```ignore
    /// let output = CommandBuilder::git_command(&repo_path, &["status", "--porcelain"])?;
    /// let stdout = String::from_utf8(output.stdout)?;
    /// ```
    pub fn git_command<P: AsRef<Path>>(cwd: P, args: &[&str]) -> io::Result<Output> {
        let cwd = cwd.as_ref();
        let mut cmd = Command::new("git");
        cmd.current_dir(cwd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd.output()
    }

    /// Run a `git` subcommand with inherited stdio so it streams straight to the
    /// user's terminal (e.g. an interactive `git diff` / `git status` / `git log`
    /// the user reads on screen). Unlike [`git_command`], output is not captured.
    /// Spawn errors and non-zero exits are ignored, matching the call sites.
    pub fn git_interactive<P: AsRef<Path>>(cwd: P, args: &[&str]) {
        let _ = Command::new("git")
            .args(args)
            .current_dir(cwd.as_ref())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();
    }

    /// Run fusermount/umount in multiple fallback variants to unmount a FUSE filesystem.
    ///
    /// Tries all common unmount methods in order until one succeeds. This is a best-effort
    /// operation — succeeds if any variant returns success.
    ///
    /// # Arguments
    /// * `mount_point` - Path to unmount
    ///
    /// # Returns
    /// * `Ok(())` if any unmount variant succeeded
    /// * `Err` if all variants failed
    pub fn unmount_archive<P: AsRef<Path>>(mount_point: P) -> io::Result<()> {
        let mount_point = mount_point.as_ref();
        let path_str = mount_point.to_string_lossy();

        let variants: &[(&str, &[&str])] = &[
            ("fusermount", &["-u", &path_str]),
            ("fusermount3", &["-u", &path_str]),
            ("fusermount", &["-uz", &path_str]),
            ("fusermount3", &["-uz", &path_str]),
            ("umount", &[&path_str]),
            ("umount", &["-l", &path_str]),
        ];

        for (tool, args) in variants {
            if let Ok(status) = Command::new(tool)
                .args(*args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                && status.success()
            {
                return Ok(());
            }
        }

        Err(io::Error::other(format!("failed to unmount {}", path_str)))
    }

    /// Append the `wget`/`curl` arguments that download `url` to `output_path`,
    /// requesting a progress bar on stderr. Returns an error for tools other
    /// than `wget`/`curl`.
    fn apply_download_args(
        cmd: &mut Command,
        tool: &str,
        url: &str,
        output_path: &Path,
    ) -> Result<(), String> {
        match tool {
            "wget" => {
                cmd.arg("--progress=bar:force")
                    .arg("--output-document")
                    .arg(output_path)
                    .arg("--")
                    .arg(url);
            }
            "curl" => {
                cmd.args(["--location", "--fail", "--output"])
                    .arg(output_path)
                    .arg("--progress-bar")
                    .arg(url);
            }
            other => return Err(format!("unsupported download tool: {}", other)),
        }
        Ok(())
    }

    /// Download from `url` to `output_path` using `wget` or `curl`, streaming stderr for progress.
    ///
    /// `on_progress` receives a short snippet suitable for a status/footer line. Callbacks are
    /// throttled and coalesced so callers can forward them to a UI without flooding.
    pub fn download_with_progress<F>(
        tool: &str,
        url: &str,
        output_path: &Path,
        mut on_progress: F,
    ) -> Result<(), String>
    where
        F: FnMut(&str),
    {
        fn stderr_progress_tail(buf: &[u8]) -> Option<String> {
            let s = String::from_utf8_lossy(buf);
            let piece = s
                .rsplit('\r')
                .next()
                .unwrap_or("")
                .rsplit('\n')
                .next()
                .unwrap_or("")
                .trim();
            if piece.is_empty() {
                return None;
            }
            const MAX: usize = 160;
            let count = piece.chars().count();
            let out: String = if count <= MAX {
                piece.to_string()
            } else {
                piece.chars().skip(count.saturating_sub(MAX)).collect()
            };
            Some(out)
        }

        let mut cmd = Command::new(tool);
        Self::apply_download_args(&mut cmd, tool, url, output_path)?;

        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| "no stderr pipe".to_string())?;

        let mut raw_buf = Vec::new();
        let mut err_accum = String::new();
        let mut read_buf = [0u8; 4096];
        let mut last_emit = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        let throttle = Duration::from_millis(200);
        let mut last_hint: Option<String> = None;

        loop {
            match stderr.read(&mut read_buf) {
                Ok(0) => break,
                Ok(n) => {
                    raw_buf.extend_from_slice(&read_buf[..n]);
                    if raw_buf.len() > 16 * 1024 {
                        let drain = raw_buf.len().saturating_sub(16 * 1024);
                        raw_buf.drain(..drain);
                    }
                    let chunk = String::from_utf8_lossy(&read_buf[..n]);
                    err_accum.push_str(&chunk);
                    if err_accum.len() > 48 * 1024 {
                        err_accum.drain(..err_accum.len().saturating_sub(48 * 1024));
                    }

                    if let Some(s) = stderr_progress_tail(&raw_buf) {
                        let elapsed_ok = last_emit.elapsed() >= throttle;
                        let changed = last_hint.as_deref() != Some(s.as_str());
                        if elapsed_ok || changed {
                            on_progress(&s);
                            last_emit = Instant::now();
                            last_hint = Some(s);
                        }
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            let tail = err_accum.trim();
            let detail = if tail.is_empty() {
                format!("{} exited with status {}", tool, status)
            } else {
                let t = tail
                    .rsplit('\r')
                    .next()
                    .unwrap_or("")
                    .rsplit('\n')
                    .next()
                    .unwrap_or("")
                    .trim();
                let tc = t.chars().count();
                let t = if tc > 512 {
                    t.chars().skip(tc.saturating_sub(512)).collect::<String>()
                } else {
                    t.to_string()
                };
                format!("{}: {}", tool, t)
            };
            Err(detail)
        }
    }
}

#[cfg(all(test, unix))]
mod timeout_tests {
    use super::*;

    #[test]
    fn output_with_timeout_captures_and_kills() {
        let mut ok = Command::new("sh");
        ok.args(["-c", "echo out; echo err >&2"]);
        let out = output_with_timeout(ok, Duration::from_secs(5)).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "out\n");
        assert_eq!(String::from_utf8_lossy(&out.stderr), "err\n");

        // The grandchild `sleep` inherits the pipes; it must die too or this hangs.
        let mut slow = Command::new("sh");
        slow.args(["-c", "sleep 30 | cat"]);
        let start = Instant::now();
        let err = output_with_timeout(slow, Duration::from_millis(200)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_shell_words_handles_args_and_quotes() {
        let w = |s: &str| split_shell_words(s).unwrap();
        assert_eq!(w("nano"), ["nano"]);
        assert_eq!(w("  code   -w "), ["code", "-w"]);
        assert_eq!(w("nvim -u 'my init.lua'"), ["nvim", "-u", "my init.lua"]);
        assert_eq!(
            w(r#""/opt/My Editor/ed" --wait"#),
            ["/opt/My Editor/ed", "--wait"]
        );
        assert_eq!(w(r"/opt/My\ Editor/ed"), ["/opt/My Editor/ed"]);
        assert_eq!(split_shell_words("vim 'oops"), None);
    }

    #[test]
    fn run_interactive_reports_missing_binary() {
        let err = run_interactive(&mut Command::new("/nonexistent/sbrs-no-such-tool")).unwrap_err();
        assert_eq!(err, "sbrs-no-such-tool not found");
        let err = run_interactive(&mut Command::new("sbrs-no-such-tool")).unwrap_err();
        assert_eq!(err, "sbrs-no-such-tool not found in PATH");
    }

    #[cfg(unix)]
    #[test]
    fn run_interactive_reports_exit_status() {
        assert!(run_interactive(&mut Command::new("true")).is_ok());
        let err = run_interactive(Command::new("sh").args(["-c", "exit 3"])).unwrap_err();
        assert_eq!(err, "sh exited with status 3");
    }

    /// A Ctrl+C while an interactive tool runs reaches sb too (same process
    /// group, raw mode off); sb must survive it. If this regresses, the
    /// SIGINT kills the test binary itself.
    #[cfg(unix)]
    #[test]
    fn run_interactive_survives_sigint_to_parent() {
        let err = run_interactive(Command::new("sh").args(["-c", "kill -INT $PPID; exit 4"]))
            .unwrap_err();
        assert_eq!(err, "sh exited with status 4");
    }
}
