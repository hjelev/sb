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
        PreviewLineKind::Styled { fg, bold, dim, icon } => {
            let modify = |mut style: Style| {
                if bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if dim {
                    style = style.add_modifier(Modifier::DIM);
                }
                style
            };
            let name_style = modify(Style::default().fg(fg.unwrap_or(Palette::TEXT_NORMAL)));
            match icon {
                Some(span) if span.len <= line.len() && line.is_char_boundary(span.len) => {
                    let (icon_part, name_part) = line.split_at(span.len);
                    let icon_style =
                        modify(Style::default().fg(span.fg.unwrap_or(Palette::TEXT_NORMAL)));
                    Line::from(vec![
                        Span::styled(icon_part.to_string(), icon_style),
                        Span::styled(name_part.to_string(), name_style),
                    ])
                }
                _ => Line::from(Span::styled(line.to_string(), name_style)),
            }
        }
    }
}