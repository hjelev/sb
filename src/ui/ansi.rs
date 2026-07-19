use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

/// Parse ANSI SGR color codes in a line and return styled ratatui spans.
/// Supports: 0 (reset), 1 (bold), 30-37 (fg), 90-97 (bright fg),
/// 39/49 (default fg/bg), 40-47 / 100-107 (bg), 38;5;N / 48;5;N (256-color),
/// 38;2;R;G;B / 48;2;R;G;B (truecolor). Non-SGR CSI sequences (cursor
/// visibility, erase, ...) are consumed and discarded instead of leaking
/// into the text.
pub fn parse_ansi_line(line: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_text = String::new();
    let mut current_bold = false;
    let mut current_fg: Option<Color> = None;
    let mut current_bg: Option<Color> = None;

    let flush = |text: &mut String,
                 spans: &mut Vec<Span<'static>>,
                 bold: bool,
                 fg: Option<Color>,
                 bg: Option<Color>| {
        if text.is_empty() {
            return;
        }
        let mut style = Style::default();
        style = style.fg(fg.unwrap_or(Color::Rgb(210, 210, 210)));
        if let Some(bg) = bg {
            style = style.bg(bg);
        }
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(std::mem::take(text), style));
    };

    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            let mut code_str = String::new();
            let mut is_sgr = false;
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_ascii_digit() || next_ch == ';' || next_ch == ':' || next_ch == '?' {
                    code_str.push(next_ch);
                    chars.next();
                } else {
                    // CSI final byte: consume it; only SGR ('m') is interpreted.
                    is_sgr = next_ch == 'm';
                    chars.next();
                    break;
                }
            }

            if !is_sgr || code_str.starts_with('?') {
                continue;
            }

            flush(&mut current_text, &mut spans, current_bold, current_fg, current_bg);

            // Parse SGR codes: split by semicolon to handle multi-part codes
            let parts: Vec<&str> = code_str.split(';').collect();
            let mut i = 0;
            while i < parts.len() {
                if let Ok(code) = parts[i].parse::<u16>() {
                    match code {
                        0 => {
                            current_bold = false;
                            current_fg = None;
                            current_bg = None;
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
                        39 => current_fg = None,
                        90 => current_fg = Some(Color::DarkGray),
                        91 => current_fg = Some(Color::Red),
                        92 => current_fg = Some(Color::Green),
                        93 => current_fg = Some(Color::Yellow),
                        94 => current_fg = Some(Color::Blue),
                        95 => current_fg = Some(Color::Magenta),
                        96 => current_fg = Some(Color::Cyan),
                        97 => current_fg = Some(Color::White),
                        // Basic backgrounds (40-47, 100-107 bright)
                        40 | 100 => current_bg = Some(Color::Black),
                        41 | 101 => current_bg = Some(Color::Red),
                        42 | 102 => current_bg = Some(Color::Green),
                        43 | 103 => current_bg = Some(Color::Yellow),
                        44 | 104 => current_bg = Some(Color::Blue),
                        45 | 105 => current_bg = Some(Color::Magenta),
                        46 | 106 => current_bg = Some(Color::Cyan),
                        47 | 107 => current_bg = Some(Color::White),
                        49 => current_bg = None,
                        // 256-color mode: 38;5;N / 48;5;N
                        38 | 48 if i + 2 < parts.len() && parts[i + 1] == "5" => {
                            if let Ok(color_idx) = parts[i + 2].parse::<u8>() {
                                if code == 38 {
                                    current_fg = Some(Color::Indexed(color_idx));
                                } else {
                                    current_bg = Some(Color::Indexed(color_idx));
                                }
                            }
                            i += 2;
                        }
                        // Truecolor: 38;2;R;G;B / 48;2;R;G;B
                        38 | 48 if i + 4 < parts.len() && parts[i + 1] == "2" => {
                            if let (Ok(r), Ok(g), Ok(b)) = (
                                parts[i + 2].parse::<u8>(),
                                parts[i + 3].parse::<u8>(),
                                parts[i + 4].parse::<u8>(),
                            ) {
                                if code == 38 {
                                    current_fg = Some(Color::Rgb(r, g, b));
                                } else {
                                    current_bg = Some(Color::Rgb(r, g, b));
                                }
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
    flush(&mut current_text, &mut spans, current_bold, current_fg, current_bg);

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}
