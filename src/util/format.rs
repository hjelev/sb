use std::time::SystemTime;
use chrono::{DateTime, Local};
use unicode_width::UnicodeWidthStr;

/// Truncate `s` to at most `max_width` terminal columns, appending `…` when
/// anything was cut. Width-aware (CJK/emoji count as 2 columns); use this when
/// the result must fit a fixed-width cell. The single source of truth for the
/// clip-with-ellipsis logic previously reimplemented across the render code.
pub fn truncate_display_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let full_width = UnicodeWidthStr::width(s);
    if full_width <= max_width {
        return s.to_string();
    }
    if max_width == 1 {
        return "…".to_string();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for ch in s.chars() {
        let ch_s = ch.to_string();
        let ch_width = UnicodeWidthStr::width(ch_s.as_str());
        if used + ch_width >= max_width {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push('…');
    out
}

/// Truncate `s` to at most `max` characters, appending `…` when anything was
/// cut. Char-count based — cheaper than [`truncate_display_width`] but only
/// correct for content that is effectively single-column.
pub fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_string();
    }
    let mut out = String::new();
    for ch in s.chars().take(max - 1) {
        out.push(ch);
    }
    out.push('…');
    out
}

/// Formats a `SystemTime` as `"YYYY-MM-DD HH:MM"` in local time.
pub fn format_mtime(t: SystemTime) -> String {
    DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string()
}

pub fn format_eta(total_seconds: u64) -> String {
    let mins = total_seconds / 60;
    let secs = total_seconds % 60;
    if mins > 0 {
        format!("{}m{:02}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Renders a textual progress bar of `width` characters: `#` for the filled
/// portion (`percent`% of `width`, rounded and clamped) and `-` for the rest.
///
/// Centralizes the identical bar-building used by the copy and archive progress
/// status lines.
pub fn progress_bar(percent: f64, width: usize) -> String {
    let filled = (((percent / 100.0) * width as f64).round() as usize).min(width);
    format!("{}{}", "#".repeat(filled), "-".repeat(width.saturating_sub(filled)))
}

pub fn format_size(bytes: u64) -> String {
    let units = ["B", "K", "M", "G", "T"];
    let mut size = bytes as f64;
    let mut unit_idx = 0usize;
    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{}{}", bytes, units[unit_idx])
    } else if size >= 10.0 {
        format!("{:.0}{}", size, units[unit_idx])
    } else {
        format!("{:.1}{}", size, units[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar() {
        assert_eq!(progress_bar(0.0, 4), "----");
        assert_eq!(progress_bar(50.0, 4), "##--");
        assert_eq!(progress_bar(100.0, 4), "####");
        // Over 100% is clamped to the bar width.
        assert_eq!(progress_bar(150.0, 4), "####");
    }
}
