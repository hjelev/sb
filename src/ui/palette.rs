//! Color theme constants and palette.
//!
//! Centralizes all color definitions to replace ~40 hard-coded Color::Rgb(...) values
//! scattered throughout the codebase. Provides a single source of truth for the theme.
#![allow(dead_code)]

use ratatui::style::Color;

/// Application color palette.
///
/// All colors are 24-bit RGB values. Theme colors are grouped by semantic use:
/// - **Text colors**: Basic text rendering
/// - **Accent colors**: Highlights, selections, important elements
/// - **Status colors**: Success, warning, error states
/// - **UI colors**: Panels, borders, dividers
#[derive(Debug, Clone, Copy)]
pub struct Palette;

impl Palette {
    // ===== Neutral / Text Colors =====

    /// Bright text (default foreground)
    pub const TEXT_BRIGHT: Color = Color::Rgb(220, 220, 220);

    /// Normal text
    pub const TEXT_NORMAL: Color = Color::Rgb(210, 210, 210);

    /// Dimmed/secondary text
    pub const TEXT_DIM: Color = Color::Rgb(150, 150, 150);

    /// Very dim text (nearly invisible)
    pub const TEXT_VERY_DIM: Color = Color::Rgb(120, 120, 120);

    /// Muted/disabled text
    pub const TEXT_MUTED: Color = Color::Rgb(100, 100, 100);

    // ===== Accent Colors =====

    /// Primary accent (used for highlights, selections, important UI)
    pub const ACCENT_PRIMARY: Color = Color::Rgb(100, 160, 240);

    /// Secondary accent (used for branches, tags, alt selections)
    pub const ACCENT_SECONDARY: Color = Color::Rgb(100, 150, 255);

    /// Tertiary accent (used for borders, dividers, subtle highlights)
    pub const ACCENT_TERTIARY: Color = Color::Rgb(80, 200, 180);

    // ===== Status Colors =====

    /// Success / positive action (added files, tags)
    pub const SUCCESS: Color = Color::Rgb(100, 220, 120);

    /// Success (alternate shade, used for marks)
    pub const SUCCESS_ALT: Color = Color::Rgb(120, 220, 120);

    /// Warning / caution state (modified files)
    pub const WARNING: Color = Color::Rgb(245, 200, 90);

    /// Warning (alternate - more orange)
    pub const WARNING_ALT: Color = Color::Rgb(230, 190, 90);

    /// Warning (bright - current directory)
    pub const WARNING_BRIGHT: Color = Color::Rgb(255, 220, 120);

    /// Error / danger (deleted, removed, errors)
    pub const ERROR: Color = Color::Rgb(220, 80, 80);

    // ===== Special Colors =====

    /// Information (file type indicators, icons)
    pub const INFO: Color = Color::Rgb(100, 220, 220);

    /// Symlink color (cyan)
    pub const SYMLINK: Color = Color::Rgb(100, 220, 220);

    /// Directory color (same as ACCENT_PRIMARY)
    pub const DIRECTORY: Color = Color::Rgb(100, 160, 240);

    /// Metadata text (owner, group)
    pub const METADATA: Color = Color::Rgb(196, 172, 118);

    /// Metadata alternate (group)
    pub const METADATA_ALT: Color = Color::Rgb(172, 136, 98);

    // ===== UI/Layout Colors =====

    /// Background for selected item
    pub const BG_SELECTED: Color = Color::Rgb(50, 50, 50);

    /// Background for marked item
    pub const BG_MARKED: Color = Color::Rgb(0, 100, 150);

    /// Panel/box background
    pub const BG_PANEL: Color = Color::Rgb(60, 60, 60);

    /// Panel text on dark background
    pub const TEXT_ON_PANEL: Color = Color::White;

    /// Divider/separator
    pub const DIVIDER: Color = Color::Rgb(120, 200, 190);

    /// Border/frame
    pub const BORDER: Color = Color::Rgb(140, 140, 140);

    /// Inactive element
    pub const INACTIVE: Color = Color::Rgb(140, 140, 140);

    /// Field/input background
    pub const INPUT_BG: Color = Color::Rgb(60, 60, 60);

    /// Input foreground
    pub const INPUT_FG: Color = Color::Rgb(220, 220, 220);

    // ===== Composite Colors (use these in most code) =====

    /// Default text style (normal text)
    pub fn default_text() -> ratatui::style::Style {
        ratatui::style::Style::default().fg(Self::TEXT_NORMAL)
    }

    /// Dimmed text style (secondary information)
    pub fn dim_text() -> ratatui::style::Style {
        ratatui::style::Style::default().fg(Self::TEXT_DIM)
    }

    /// Highlight style (selected, important)
    pub fn highlight() -> ratatui::style::Style {
        ratatui::style::Style::default().fg(Self::ACCENT_PRIMARY)
    }

    /// Error style (danger, deleted)
    pub fn error() -> ratatui::style::Style {
        ratatui::style::Style::default().fg(Self::ERROR)
    }

    /// Success style (added, positive)
    pub fn success() -> ratatui::style::Style {
        ratatui::style::Style::default().fg(Self::SUCCESS)
    }

    /// Warning style (modified, caution)
    pub fn warning() -> ratatui::style::Style {
        ratatui::style::Style::default().fg(Self::WARNING)
    }

    /// Selection background style
    pub fn selection_bg() -> ratatui::style::Style {
        ratatui::style::Style::default().bg(Self::BG_SELECTED)
    }

    /// Marked item background style
    pub fn marked_bg() -> ratatui::style::Style {
        ratatui::style::Style::default().bg(Self::BG_MARKED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_colors_are_valid_rgb() {
        // Just verify all the color constants are valid RGB
        let _ = Palette::TEXT_BRIGHT;
        let _ = Palette::ACCENT_PRIMARY;
        let _ = Palette::ERROR;
        let _ = Palette::default_text();
    }
}
