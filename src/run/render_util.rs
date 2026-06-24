use super::*;

/// Darken a color toward black (mirrors `ui::panels::darken`): used for the
/// free segment of the disk pill so it reads as a darker shade of the used fill.
pub(crate) fn darken_color(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * factor) as u8,
            (g as f32 * factor) as u8,
            (b as f32 * factor) as u8,
        ),
        _ => Color::Rgb(24, 24, 30),
    }
}

/// Build one span of the disk progress bar. The "used" portion gets the
/// threshold fill color as background with a dark foreground for contrast; the
/// "free" portion uses the dim panel background with normal-text foreground.
pub(crate) fn bar_span(text: String, in_used: bool, used_bg: Option<Color>, free_bg: Color, free_fg: Color) -> Span<'static> {
    match (in_used, used_bg) {
        (true, Some(bg)) => Span::styled(text, Style::default().bg(bg).fg(Color::Black)),
        _ => Span::styled(text, Style::default().bg(free_bg).fg(free_fg)),
    }
}

pub(crate) fn truncate_display_width(s: &str, max_width: usize) -> String {
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

pub(crate) fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out = String::new();
    for ch in s.chars().take(max - 1) {
        out.push(ch);
    }
    out.push('…');
    out
}

/// The display-style inputs a panel title needs: the active theme plus the
/// icon-rendering flags. Grouping them keeps `build_panel_title`'s signature
/// small (and avoids the `too_many_arguments` lint). Build one with
/// [`title_style`].
pub(crate) struct TitleStyle {
    theme: crate::ui::theme::ThemeSpec,
    show_icons: bool,
    nerd_font: bool,
    theme_id: crate::ui::theme::ThemeId,
}

pub(crate) fn title_style(app: &App, theme: crate::ui::theme::ThemeSpec) -> TitleStyle {
    TitleStyle {
        theme,
        show_icons: app.show_icons,
        nerd_font: app.nerd_font_active,
        theme_id: app.active_theme,
    }
}

pub(crate) fn build_panel_title(
    path: &std::path::Path,
    path_text: String,
    editing: bool,
    title_width: u16,
    style: &TitleStyle,
) -> Line<'static> {
    let is_symlink = crate::util::classify::is_symlink(path);
    let (folder_icon, folder_icon_style) = App::icon_for_path(
        path,
        style.show_icons,
        style.nerd_font,
        is_symlink,
        style.theme_id,
    );
    let title_inner_width = title_width.saturating_sub(2) as usize;
    let icon_width = if folder_icon.is_empty() {
        0
    } else {
        UnicodeWidthStr::width(folder_icon.as_str()) + 1
    };
    let prefix_width = 1 + icon_width;
    let text_max_width = title_inner_width.saturating_sub(prefix_width).max(1);
    let display_text = truncate_display_width(&path_text, text_max_width);
    let mut title_spans: Vec<Span> = Vec::new();
    title_spans.push(Span::raw(" "));
    if !folder_icon.is_empty() {
        title_spans.push(Span::styled(folder_icon, folder_icon_style));
        title_spans.push(Span::raw(" "));
    }
    if editing {
        title_spans.push(Span::styled(
            display_text.clone(),
            Style::default().fg(Color::Rgb(255, 220, 120)),
        ));
    } else {
        title_spans.push(Span::styled(display_text.clone(), Style::default().fg(style.theme.text_normal)));
    }
    let used_width = prefix_width + UnicodeWidthStr::width(display_text.as_str());
    if used_width < title_inner_width {
        title_spans.push(Span::raw(" "));
    }
    Line::from(title_spans)
}

