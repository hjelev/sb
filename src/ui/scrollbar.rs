use ratatui::{
    prelude::{Color, Frame, Rect, Style},
    text::Span,
    widgets::Paragraph,
};

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
