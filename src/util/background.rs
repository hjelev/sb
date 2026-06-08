use std::sync::mpsc::{self, Receiver, TryRecvError};

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
