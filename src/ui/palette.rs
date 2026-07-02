//! Theme-independent color constants and contrast helpers.
//!
//! Most colors come from the active theme (`ui::theme`); the few constants
//! here are fallbacks used where no `ThemeSpec` is in scope.

use ratatui::style::Color;

/// Theme-independent fallback colors.
#[derive(Debug, Clone, Copy)]
pub struct Palette;

impl Palette {
    /// Normal text
    pub const TEXT_NORMAL: Color = Color::Rgb(210, 210, 210);

    /// Success (alternate shade, used for marks)
    pub const SUCCESS_ALT: Color = Color::Rgb(120, 220, 120);

    /// Warning (alternate - more orange)
    pub const WARNING_ALT: Color = Color::Rgb(230, 190, 90);

    /// Symlink color (cyan)
    pub const SYMLINK: Color = Color::Rgb(100, 220, 220);

    /// Background for marked item
    pub const BG_MARKED: Color = Color::Rgb(0, 100, 150);
}

/// Whether `bg` is a light color that needs dark foreground text for contrast.
///
/// Non-RGB colors (terminal default / ANSI names) can't be measured, so they
/// are treated as dark (themes that use them are dark-background themes).
pub fn is_light_bg(bg: Color) -> bool {
    match bg {
        // Perceived luminance (Rec. 601), range 0..=255.
        Color::Rgb(r, g, b) => 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 150.0,
        _ => false,
    }
}

/// Returns a foreground readable on `bg`: `dark` for light backgrounds,
/// `light` for dark backgrounds.
pub fn readable_fg(bg: Color, dark: Color, light: Color) -> Color {
    if is_light_bg(bg) { dark } else { light }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_fg_picks_by_luminance() {
        // Bright yellow (cyberpunk-neon selection bg) → dark text.
        assert_eq!(
            readable_fg(Color::Rgb(0xFC, 0xEE, 0x0A), Color::Black, Color::White),
            Color::Black
        );
        // Dark olive (darkened label segment) → light text.
        assert_eq!(
            readable_fg(Color::Rgb(0x8C, 0x83, 0x05), Color::Black, Color::White),
            Color::White
        );
        // Unmeasurable terminal default → light fallback.
        assert_eq!(
            readable_fg(Color::Reset, Color::Black, Color::White),
            Color::White
        );
    }
}
