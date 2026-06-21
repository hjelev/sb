//! Small helpers for moving a list-selection cursor within overlays.
//!
//! Overlay panels (integrations, bookmarks, sort menu, themes, search results,
//! …) each keep their selected row as a plain `usize` on `App`. Moving that
//! cursor up/down with clamping was duplicated across both the keyboard
//! (`run/key_dispatch`) and mouse (`app_mouse`) handlers; these two functions
//! centralize the clamp logic so the call sites read as intent, not arithmetic.

/// Move a list cursor up one row, saturating at the first row.
pub fn cursor_up(selected: &mut usize) {
    *selected = selected.saturating_sub(1);
}

/// Move a list cursor down one row, clamped to the last valid index for a list
/// of `len` items. With an empty list the cursor stays at 0.
pub fn cursor_down(selected: &mut usize, len: usize) {
    *selected = (*selected + 1).min(len.saturating_sub(1));
}
