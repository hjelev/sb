use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::PreviewLineKind;

use super::palette::Palette;

pub fn render_directory_preview_line(line: &str, kind: Option<PreviewLineKind>) -> Line<'static> {
    match kind.unwrap_or(PreviewLineKind::Plain) {
        PreviewLineKind::Plain => Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Palette::TEXT_NORMAL),
        )),
        PreviewLineKind::Styled { fg, bold, dim } => {
            let mut style = Style::default().fg(fg.unwrap_or(Palette::TEXT_NORMAL));
            if bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if dim {
                style = style.add_modifier(Modifier::DIM);
            }
            Line::from(Span::styled(line.to_string(), style))
        }
    }
}