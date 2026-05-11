//! Span construction and styling helpers.
//!
//! Replaces ~40 individual `Span::styled()` calls with reusable helper functions.
//! Uses palette constants for consistent styling.
#![allow(dead_code)]

use ratatui::text::Span;
use ratatui::style::{Style, Modifier};
use super::palette::Palette;

/// Build a titled span (title: content format).
///
/// Common pattern: "Key: value" or "Mode: Browse"
///
/// # Arguments
/// * `title` - The label/title part (styled dim)
/// * `content` - The value/content part (styled bright)
/// * `separator` - Separator between title and content (default ": ")
///
/// # Returns
/// * `Vec<Span>` - Ready to render or combine with other spans
///
/// # Example
/// ```ignore
/// let spans = titled_span("Branch", "main", ": ");
/// // Produces: dim("Branch") + dim(": ") + bright("main")
/// ```
pub fn titled_span(title: &str, content: &str, separator: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(title.to_string(), Palette::dim_text()),
        Span::styled(separator.to_string(), Palette::dim_text()),
        Span::styled(content.to_string(), Palette::default_text()),
    ]
}

/// Build a status span (icon + message).
///
/// # Arguments
/// * `message` - Status message
/// * `icon` - Unicode icon or symbol
/// * `status_type` - Type of status (success, error, warning)
///
/// # Returns
/// * `Span` - Single styled span
pub fn status_span(message: &str, icon: &str, status_type: StatusType) -> Span<'static> {
    let style = match status_type {
        StatusType::Success => Palette::success(),
        StatusType::Error => Palette::error(),
        StatusType::Warning => Palette::warning(),
        StatusType::Info => Palette::highlight(),
    };
    Span::styled(format!("{} {}", icon, message), style)
}

/// Build a file/entry type span (colored by type).
///
/// # Arguments
/// * `name` - File or directory name
/// * `entry_type` - Type of entry (file, dir, symlink, archive, etc.)
///
/// # Returns
/// * `Span` - Colored span based on entry type
pub fn entry_type_span(name: &str, entry_type: EntryType) -> Span<'_> {
    let style = match entry_type {
        EntryType::Directory => Style::default().fg(Palette::DIRECTORY),
        EntryType::Symlink => Style::default().fg(Palette::ACCENT_SECONDARY),
        EntryType::Archive => Style::default().fg(Palette::WARNING),
        EntryType::Executable => Style::default().fg(Palette::SUCCESS).add_modifier(Modifier::BOLD),
        EntryType::Modified => Style::default().fg(Palette::WARNING),
        EntryType::Added => Style::default().fg(Palette::SUCCESS),
        EntryType::Deleted => Style::default().fg(Palette::ERROR),
        EntryType::File => Palette::default_text(),
    };
    Span::styled(name, style)
}

/// Build a dimmed/secondary text span.
pub fn dim_span(text: &str) -> Span<'_> {
    Span::styled(text, Palette::dim_text())
}

/// Build a highlighted span.
pub fn highlight_span(text: &str) -> Span<'_> {
    Span::styled(text, Palette::highlight())
}

/// Build an error-colored span.
pub fn error_span(text: &str) -> Span<'_> {
    Span::styled(text, Palette::error())
}

/// Build a success-colored span.
pub fn success_span(text: &str) -> Span<'_> {
    Span::styled(text, Palette::success())
}

/// Build a warning-colored span.
pub fn warning_span(text: &str) -> Span<'_> {
    Span::styled(text, Palette::warning())
}

/// Status type for status spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusType {
    Success,
    Error,
    Warning,
    Info,
}

/// Entry type for colorizing file/directory names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    File,
    Directory,
    Symlink,
    Archive,
    Executable,
    Modified,
    Added,
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_titled_span() {
        let spans = titled_span("Mode", "Browse", ": ");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "Mode");
        assert_eq!(spans[1].content, ": ");
        assert_eq!(spans[2].content, "Browse");
    }

    #[test]
    fn test_status_span() {
        let span = status_span("File saved", "✓", StatusType::Success);
        assert!(span.content.contains("File saved"));
    }

    #[test]
    fn test_entry_type_span() {
        let span = entry_type_span("folder", EntryType::Directory);
        assert_eq!(span.content, "folder");
    }
}
