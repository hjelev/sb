//! Generic background task pipeline abstraction.
//!
//! Replaces multiple separate `pump_*_progress()` methods with a single generic
//! abstraction for managing background task channels.
#![allow(dead_code)]

use std::sync::mpsc::Receiver;
use std::marker::PhantomData;

/// Generic background task handler.
///
/// Wraps a crossbeam channel receiver for consistent polling of background task results.
/// Replaces patterns like multiple separate `pump_*_progress()` methods.
///
/// # Type Parameters
/// * `T` - Result type produced by the background task
///
/// # Example (from event loop)
/// ```ignore
/// // In main event loop instead of 8 separate pump_* methods:
/// while let Ok(git_result) = self.git_task.try_recv() {
///     self.handle_git_result(git_result);
/// }
/// while let Ok(size_result) = self.folder_size_task.try_recv() {
///     self.handle_folder_size_result(size_result);
/// }
/// ```
pub struct BackgroundTask<T> {
    receiver: Option<Receiver<T>>,
    _phantom: PhantomData<T>,
}

impl<T> BackgroundTask<T> {
    /// Create a new background task from a receiver.
    pub fn new(receiver: Receiver<T>) -> Self {
        Self {
            receiver: Some(receiver),
            _phantom: PhantomData,
        }
    }

    /// Create an inactive background task (no receiver).
    pub fn inactive() -> Self {
        Self {
            receiver: None,
            _phantom: PhantomData,
        }
    }

    /// Check if this task has a receiver (is active).
    pub fn is_active(&self) -> bool {
        self.receiver.is_some()
    }

    /// Try to receive the next result without blocking.
    ///
    /// Returns:
    /// * `Ok(result)` if a result is available
    /// * `Err` if the channel is empty or disconnected
    pub fn try_recv(&self) -> Result<T, String> {
        match &self.receiver {
            None => Err("Task is inactive".to_string()),
            Some(rx) => rx.try_recv()
                .map_err(|e| e.to_string()),
        }
    }

    /// Replace the receiver with a new one.
    pub fn set_receiver(&mut self, receiver: Receiver<T>) {
        self.receiver = Some(receiver);
    }

    /// Clear the receiver (deactivate the task).
    pub fn clear(&mut self) {
        self.receiver = None;
    }
}

// NOTE: BackgroundTask<T> intentionally does not implement Clone because
// std::sync::mpsc::Receiver<T> is not Clone.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_background_task_inactive() {
        let task: BackgroundTask<String> = BackgroundTask::inactive();
        assert!(!task.is_active());
        assert!(task.try_recv().is_err());
    }

    #[test]
    fn test_background_task_receive() {
        let (tx, rx) = mpsc::channel();
        let task: BackgroundTask<i32> = BackgroundTask::new(rx);

        assert!(task.is_active());

        tx.send(42).unwrap();
        let result = task.try_recv().unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_background_task_empty() {
        let (_tx, rx) = mpsc::channel::<i32>();
        let task: BackgroundTask<i32> = BackgroundTask::new(rx);

        assert!(task.is_active());
        assert!(task.try_recv().is_err()); // No data yet
    }
}
