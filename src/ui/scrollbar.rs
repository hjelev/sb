use ratatui::{
    prelude::{Color, Frame, Rect, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// Thumb geometry shared by every scrollbar in the app. Rendering and mouse
/// hit-testing must use the same math, or drag positions silently desync from
/// what is drawn. Returns `(thumb_y, thumb_h)` in track rows.
pub fn scrollbar_thumb(
    total_rows: usize,
    visible_rows: usize,
    offset: usize,
    track_h: usize,
) -> (usize, usize) {
    if track_h == 0 {
        return (0, 0);
    }
    if total_rows == 0 {
        return (0, track_h);
    }
    let thumb_h = (visible_rows * track_h)
        .div_ceil(total_rows)
        .max(1)
        .min(track_h);
    let max_scroll = total_rows.saturating_sub(visible_rows);
    let scroll_space = track_h.saturating_sub(thumb_h);
    let thumb_y = if max_scroll == 0 {
        0
    } else {
        (offset.min(max_scroll) * scroll_space + max_scroll / 2) / max_scroll
    };
    (thumb_y, thumb_h)
}

/// Render a vertical scrollbar track into `sb_area` using the shared thumb
/// geometry. Draws nothing when the track or content is empty.
pub fn render_scrollbar_track(
    f: &mut Frame,
    sb_area: Rect,
    total_rows: usize,
    visible_rows: usize,
    offset: usize,
    thumb_color: Color,
    track_color: Color,
) {
    let track_h = sb_area.height as usize;
    if track_h == 0 || total_rows == 0 {
        return;
    }
    let (thumb_y, thumb_h) = scrollbar_thumb(total_rows, visible_rows, offset, track_h);
    let mut sb_lines: Vec<Line> = Vec::with_capacity(track_h);
    for row in 0..track_h {
        let in_thumb = row >= thumb_y && row < thumb_y + thumb_h;
        let (ch, color) = if in_thumb {
            ("┃", thumb_color)
        } else {
            ("│", track_color)
        };
        sb_lines.push(Line::from(Span::styled(ch, Style::default().fg(color))));
    }
    f.render_widget(Paragraph::new(sb_lines), sb_area);
}

pub fn render_scrollbar_corners(
    f: &mut Frame,
    area: Rect,
    can_draw_scrollbar: bool,
    border_color: Color,
) {
    if !can_draw_scrollbar {
        return;
    }
    let corner_x = area.x + area.width.saturating_sub(1);
    let top_corner_y = area.y.saturating_sub(1);
    let bottom_corner_y = area.y + area.height;
    let corner_style = Style::default().fg(border_color);
    f.render_widget(
        Paragraph::new(Span::styled("╮", corner_style)),
        Rect::new(corner_x, top_corner_y, 1, 1),
    );
    f.render_widget(
        Paragraph::new(Span::styled("╯", corner_style)),
        Rect::new(corner_x, bottom_corner_y, 1, 1),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_fills_track_when_content_fits() {
        // 10 rows visible out of 10 total: full-height thumb at the top.
        assert_eq!(scrollbar_thumb(10, 10, 0, 10), (0, 10));
        assert_eq!(scrollbar_thumb(0, 10, 0, 10), (0, 10));
    }

    #[test]
    fn thumb_is_proportional_and_reaches_both_ends() {
        // 100 rows, 10 visible, track of 10 → thumb height 1.
        let (y0, h) = scrollbar_thumb(100, 10, 0, 10);
        assert_eq!((y0, h), (0, 1));
        // At max scroll (offset 90) the thumb sits at the bottom.
        let (y_end, h_end) = scrollbar_thumb(100, 10, 90, 10);
        assert_eq!(h_end, 1);
        assert_eq!(y_end + h_end, 10);
    }

    #[test]
    fn thumb_clamps_out_of_range_offset() {
        let at_max = scrollbar_thumb(100, 10, 90, 10);
        assert_eq!(scrollbar_thumb(100, 10, 500, 10), at_max);
    }

    #[test]
    fn thumb_handles_empty_track() {
        assert_eq!(scrollbar_thumb(100, 10, 0, 0), (0, 0));
    }
}
