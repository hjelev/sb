use std::time::SystemTime;
use chrono::{DateTime, Local};

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
