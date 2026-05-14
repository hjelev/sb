use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

/// Parse ANSI SGR color codes in a line and return styled ratatui spans.
/// Supports: 0 (reset), 1 (bold), 30-37 (fg), 90-97 (bright fg),
/// 38;5;N (256-color fg), 38;2;R;G;B (truecolor fg).
pub fn parse_ansi_line(line: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_bold = false;
    let mut current_fg: Option<Color> = None;

    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            let mut code_str = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_ascii_digit() || next_ch == ';' {
                    code_str.push(next_ch);
                    chars.next();
                } else if next_ch == 'm' {
                    chars.next();
                    break;
                } else {
                    break;
                }
            }

            // Flush current text
            if !current_text.is_empty() {
                let mut style = Style::default();
                if let Some(fg) = current_fg {
                    style = style.fg(fg);
                } else {
                    style = style.fg(Color::Rgb(210, 210, 210));
                }
                if current_bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                spans.push(Span::styled(current_text.clone(), style));
                current_text.clear();
            }

            // Parse SGR codes: split by semicolon to handle multi-part codes
            let parts: Vec<&str> = code_str.split(';').collect();
            let mut i = 0;
            while i < parts.len() {
                if let Ok(code) = parts[i].parse::<u16>() {
                    match code {
                        0 => {
                            current_bold = false;
                            current_fg = None;
                        }
                        1 => current_bold = true,
                        // Basic colors (30-37 fg, 90-97 bright fg)
                        30 => current_fg = Some(Color::Black),
                        31 => current_fg = Some(Color::Red),
                        32 => current_fg = Some(Color::Green),
                        33 => current_fg = Some(Color::Yellow),
                        34 => current_fg = Some(Color::Blue),
                        35 => current_fg = Some(Color::Magenta),
                        36 => current_fg = Some(Color::Cyan),
                        37 => current_fg = Some(Color::White),
                        90 => current_fg = Some(Color::DarkGray),
                        91 => current_fg = Some(Color::Red),
                        92 => current_fg = Some(Color::Green),
                        93 => current_fg = Some(Color::Yellow),
                        94 => current_fg = Some(Color::Blue),
                        95 => current_fg = Some(Color::Magenta),
                        96 => current_fg = Some(Color::Cyan),
                        97 => current_fg = Some(Color::White),
                        // 256-color mode: 38;5;N
                        38 if i + 2 < parts.len() && parts[i + 1] == "5" => {
                            if let Ok(color_idx) = parts[i + 2].parse::<u8>() {
                                current_fg = Some(Color::Indexed(color_idx));
                            }
                            i += 2;
                        }
                        // Truecolor: 38;2;R;G;B
                        38 if i + 4 < parts.len() && parts[i + 1] == "2" => {
                            if let (Ok(r), Ok(g), Ok(b)) = (
                                parts[i + 2].parse::<u8>(),
                                parts[i + 3].parse::<u8>(),
                                parts[i + 4].parse::<u8>(),
                            ) {
                                current_fg = Some(Color::Rgb(r, g, b));
                            }
                            i += 4;
                        }
                        _ => {}
                    }
                }
                i += 1;
            }
        } else {
            current_text.push(ch);
        }
    }

    // Flush remaining text
    if !current_text.is_empty() {
        let mut style = Style::default();
        if let Some(fg) = current_fg {
            style = style.fg(fg);
        } else {
            style = style.fg(Color::Rgb(210, 210, 210));
        }
        if current_bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(current_text, style));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}
