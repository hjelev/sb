use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

/// Spawn a background worker thread wired to a fresh channel.
///
/// Creates an `mpsc` channel, hands the `Sender` to `work` (which runs on the
/// new thread), and returns the `Receiver` for the caller to store and poll
/// with [`drain_channel`]. Replaces the hand-rolled
/// `let (tx, rx) = channel(); thread::spawn(move || { … tx.send(…) })` pattern.
///
/// # Usage pattern
/// ```ignore
/// self.notes_rx = Some(spawn_worker(move |tx| {
///     let notes = load_notes(&dir);
///     let _ = tx.send(NotesLoadMsg { dir, notes });
/// }));
/// ```
pub fn spawn_worker<T, F>(work: F) -> Receiver<T>
where
    T: Send + 'static,
    F: FnOnce(mpsc::Sender<T>) + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || work(tx));
    rx
}

/// Drain all currently-available messages from an optional channel receiver.
///
/// Puts the receiver back into `rx` if the channel is still open (sender not dropped).
/// Leaves `rx` as `None` if the sender disconnected.
///
/// # Usage pattern
/// ```ignore
/// for msg in drain_channel(&mut self.notes_rx) {
///     match msg {
///         Msg::Done(..) => { self.notes_rx = None; /* stop future polls */ }
///         Msg::Progress(..) => { /* update state */ }
///     }
/// }
/// ```
pub fn drain_channel<T>(rx: &mut Option<Receiver<T>>) -> Vec<T> {
    let Some(taken) = rx.take() else { return vec![] };
    let mut messages = Vec::new();
    let mut channel_open = true;
    loop {
        match taken.try_recv() {
            Ok(msg) => messages.push(msg),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                channel_open = false;
                break;
            }
        }
    }
    if channel_open {
        *rx = Some(taken);
    }
    messages
}

/// Lifecycle state for a cancellable background job that streams results back
/// over a channel.
///
/// Bundles the three values every such job used to track as separate fields:
/// the [`Receiver`], a monotonically increasing `scan_id` used to discard
/// results from superseded scans, and an optional cooperative-cancel token.
/// Pair it with [`drain_channel`] on `&mut self.<job>.rx`.
pub struct AsyncJobState<T> {
    pub rx: Option<Receiver<T>>,
    pub scan_id: u64,
    pub cancel: Option<Arc<AtomicBool>>,
}

impl<T> Default for AsyncJobState<T> {
    fn default() -> Self {
        Self {
            rx: None,
            scan_id: 0,
            cancel: None,
        }
    }
}

impl<T> AsyncJobState<T> {
    /// Bump the scan id and return the new value; tag worker messages with it.
    pub fn next_scan_id(&mut self) -> u64 {
        self.scan_id = self.scan_id.wrapping_add(1);
        self.scan_id
    }

    /// True when `id` matches the current scan (i.e. the message is not stale).
    pub fn is_current(&self, id: u64) -> bool {
        id == self.scan_id
    }

    /// Cancel any in-flight worker and install a fresh token for the next one.
    ///
    /// Flips the old token (if any) to `true` so a still-running walk bails out
    /// early, then returns a new token to clone into the new worker.
    pub fn renew_cancel(&mut self) -> Arc<AtomicBool> {
        self.abort_cancel();
        let token = Arc::new(AtomicBool::new(false));
        self.cancel = Some(token.clone());
        token
    }

    /// Signal the in-flight worker to stop without starting a new one.
    pub fn abort_cancel(&mut self) {
        if let Some(old) = self.cancel.take() {
            old.store(true, Ordering::Relaxed);
        }
    }

    /// Drop the receiver (stop polling) without touching the cancel token.
    pub fn clear_rx(&mut self) {
        self.rx = None;
    }
}
