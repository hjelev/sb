use std::sync::mpsc::{Receiver, TryRecvError};

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
