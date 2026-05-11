//! Modal dialog abstraction and concrete implementations.
//!
//! Centralizes the pattern for help, confirm, bookmarks, and integration dialogs.
//! Each modal implements the Modal trait for consistent event handling and rendering.
#![allow(dead_code)]

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

/// Result of modal event handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalResult {
    /// Modal is still open, no action
    Open,
    /// Modal closed with cancel/escape
    Cancelled,
    /// Modal closed with confirmation
    Confirmed,
    /// Modal closed with a specific action
    Action(u8),
}

/// Trait for modal dialogs.
///
/// Implement this trait to create new modal types (help, confirm, bookmarks, etc.).
/// Each modal handles its own:
/// - Event processing (keyboard input)
/// - Rendering (display content + border)
/// - State management (scroll position, selection, etc.)
/// - Exit reason (why it closed, what action to take)
pub trait Modal: Send + Sync {
    /// Handle a keyboard event.
    ///
    /// Called on every keystroke when modal is active.
    /// Returns `ModalResult` indicating whether modal should close and what action to take.
    fn handle_event(&mut self, event: KeyEvent) -> ModalResult;

    /// Render the modal to the frame.
    ///
    /// # Arguments
    /// * `frame` - Ratatui frame to render to
    /// * `area` - Available space for modal
    fn render(&self, frame: &mut Frame, area: Rect);

    /// Get the modal's title for display in header/status.
    fn title(&self) -> &str;

    /// Check if this modal is scrollable (has content that can overflow).
    fn is_scrollable(&self) -> bool {
        false
    }

    /// Scroll down (if scrollable).
    fn scroll_down(&mut self) {
        // Default: no-op
    }

    /// Scroll up (if scrollable).
    fn scroll_up(&mut self) {
        // Default: no-op
    }
}

/// Help modal - displays keyboard shortcuts and commands.
#[derive(Debug, Clone)]
pub struct HelpModal {
    scroll_offset: u16,
    max_offset: u16,
}

impl HelpModal {
    /// Create a new help modal.
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
            max_offset: 0,
        }
    }

    /// Set maximum scroll offset (total lines - visible lines).
    pub fn set_max_offset(&mut self, max: u16) {
        self.max_offset = max;
    }
}

impl Modal for HelpModal {
    fn handle_event(&mut self, event: KeyEvent) -> ModalResult {
        use crossterm::event::KeyCode;
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => ModalResult::Cancelled,
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up();
                ModalResult::Open
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down();
                ModalResult::Open
            }
            KeyCode::PageUp => {
                for _ in 0..5 {
                    self.scroll_up();
                }
                ModalResult::Open
            }
            KeyCode::PageDown => {
                for _ in 0..5 {
                    self.scroll_down();
                }
                ModalResult::Open
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                ModalResult::Open
            }
            KeyCode::End => {
                self.scroll_offset = self.max_offset;
                ModalResult::Open
            }
            _ => ModalResult::Open,
        }
    }

    fn render(&self, _frame: &mut Frame, _area: Rect) {
        // Rendering implementation to be connected from main.rs
        // This is a placeholder that will be called from the main render function
    }

    fn title(&self) -> &str {
        "Help"
    }

    fn is_scrollable(&self) -> bool {
        true
    }

    fn scroll_down(&mut self) {
        if self.scroll_offset < self.max_offset {
            self.scroll_offset += 1;
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}

/// Confirmation modal - asks user to confirm an action.
#[derive(Debug, Clone)]
pub struct ConfirmModal {
    title: String,
    message: String,
    button_focus: u8, // 0 = No, 1 = Yes
    scrollable: bool,
    scroll_offset: u16,
}

impl ConfirmModal {
    /// Create a new confirmation modal.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            button_focus: 0,
            scrollable: false,
            scroll_offset: 0,
        }
    }

    /// Set whether content is scrollable.
    pub fn set_scrollable(&mut self, scrollable: bool) {
        self.scrollable = scrollable;
    }

    /// Get which button is focused (0 = No/Cancel, 1 = Yes/Confirm).
    pub fn button_focus(&self) -> u8 {
        self.button_focus
    }
}

impl Modal for ConfirmModal {
    fn handle_event(&mut self, event: KeyEvent) -> ModalResult {
        use crossterm::event::KeyCode;
        match event.code {
            KeyCode::Esc => ModalResult::Cancelled,
            KeyCode::Tab | KeyCode::Right => {
                self.button_focus = 1 - self.button_focus;
                ModalResult::Open
            }
            KeyCode::Left => {
                self.button_focus = 1 - self.button_focus;
                ModalResult::Open
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.button_focus == 0 {
                    ModalResult::Cancelled
                } else {
                    ModalResult::Confirmed
                }
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => ModalResult::Confirmed,
            KeyCode::Char('n') | KeyCode::Char('N') => ModalResult::Cancelled,
            KeyCode::Up | KeyCode::Char('k') if self.scrollable => {
                self.scroll_up();
                ModalResult::Open
            }
            KeyCode::Down | KeyCode::Char('j') if self.scrollable => {
                self.scroll_down();
                ModalResult::Open
            }
            _ => ModalResult::Open,
        }
    }

    fn render(&self, _frame: &mut Frame, _area: Rect) {
        // Rendering implementation to be connected from main.rs
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn is_scrollable(&self) -> bool {
        self.scrollable
    }

    fn scroll_down(&mut self) {
        if self.scrollable {
            self.scroll_offset += 1;
        }
    }

    fn scroll_up(&mut self) {
        if self.scrollable && self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}

/// Bookmark selection modal.
#[derive(Debug, Clone)]
pub struct BookmarkModal {
    selected: usize,
    total: usize,
}

impl BookmarkModal {
    /// Create a new bookmark modal.
    pub fn new(total: usize) -> Self {
        Self { selected: 0, total }
    }

    /// Get current selection.
    pub fn selected(&self) -> usize {
        self.selected
    }
}

impl Modal for BookmarkModal {
    fn handle_event(&mut self, event: KeyEvent) -> ModalResult {
        use crossterm::event::KeyCode;
        match event.code {
            KeyCode::Esc => ModalResult::Cancelled,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                ModalResult::Open
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected < self.total.saturating_sub(1) {
                    self.selected += 1;
                }
                ModalResult::Open
            }
            KeyCode::Enter => ModalResult::Confirmed,
            _ => ModalResult::Open,
        }
    }

    fn render(&self, _frame: &mut Frame, _area: Rect) {
        // Rendering implementation to be connected from main.rs
    }

    fn title(&self) -> &str {
        "Bookmarks"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[test]
    fn test_help_modal_scroll() {
        let mut modal = HelpModal::new();
        modal.set_max_offset(10);

        assert_eq!(modal.scroll_offset, 0);
        modal.scroll_down();
        assert_eq!(modal.scroll_offset, 1);
    }

    #[test]
    fn test_confirm_modal_button_focus() {
        let mut modal = ConfirmModal::new("Confirm", "Delete file?");
        assert_eq!(modal.button_focus(), 0);
        modal.handle_event(KeyEvent::new(KeyCode::Tab, crossterm::event::KeyModifiers::NONE));
        assert_eq!(modal.button_focus(), 1);
    }
}
