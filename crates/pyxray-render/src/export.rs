//! Getting a rendered buffer out: to a terminal, to a file, to a web page.
//!
//! All four exporters read the same [`Buffer`], so what you see in the
//! terminal and what you paste into a document are the same picture.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};

use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Ansi,
    Text,
    Html,
    Svg,
}

pub fn format(id: &str) -> Option<Format> {
    match id {
        "ansi" => Some(Format::Ansi),
        "text" | "txt" | "plain" => Some(Format::Text),
        "html" => Some(Format::Html),
        "svg" => Some(Format::Svg),
        _ => None,
    }
}

// ---------------------------------------------------------------- colour ---

/// Resolve a ratatui colour to 24-bit RGB. `Reset` takes the theme's own
/// ground or ink, which is what makes the 16-colour themes exportable at all.
pub fn to_rgb(c: Color, t: &Theme, is_bg: bool) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Reset => {
            if is_bg {
                match t.pal.bg {
                    Color::Rgb(r, g, b) => (r, g, b),
                    _ => (0x12, 0x12, 0x14),
                }
            } else {
                match t.pal.fg {
                    Color::Rgb(r, g, b) => (r, g, b),
                    _ => (0xd8, 0xd8, 0xd8),
                }
            }
        }
        Color::Indexed(i) => indexed_rgb(i),
        Color::Black => indexed_rgb(0),
        Color::Red => indexed_rgb(1),
        Color::Green => indexed_rgb(2),
        Color::Yellow => indexed_rgb(3),
        Color::Blue => indexed_rgb(4),
        Color::Magenta => indexed_rgb(5),
        Color::Cyan => indexed_rgb(6),
        Color::Gray => indexed_rgb(7),
        Color::DarkGray => indexed_rgb(8),
        Color::LightRed => indexed_rgb(9),
        Color::LightGreen => indexed_rgb(10),
        Color::LightYellow => indexed_rgb(11),
        Color::LightBlue => indexed_rgb(12),
        Color::LightMagenta => indexed_rgb(13),
        Color::LightCyan => indexed_rgb(14),
        Color::White => indexed_rgb(15),
    }
}

/// The xterm-256 palette: sixteen system colours, a 6×6×6 cube, then a
/// 24-step grey ramp.
fn indexed_rgb(i: u8) -> (u8, u8, u8) {
    const SYSTEM: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00),
        (0xcd, 0x31, 0x31),
        (0x0d, 0xbc, 0x79),
        (0xe5, 0xe5, 0x10),
        (0x24, 0x72, 0xc8),
        (0xbc, 0x3f, 0xbc),
        (0x11, 0xa8, 0xcd),
        (0xd0, 0xd0, 0xd0),
        (0x66, 0x66, 0x66),
        (0xf1, 0x4c, 0x4c),
        (0x23, 0xd1, 0x8b),
        (0xf5, 0xf5, 0x43),
        (0x3b, 0x8e, 0xea),
        (0xd6, 0x70, 0xd6),
        (0x29, 0xb8, 0xdb),
        (0xff, 0xff, 0xff),
    ];
    match i {
        0..=15 => SYSTEM[i as usize],
        16..=231 => {
            let n = i - 16;
            let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (step(n / 36), step((n % 36) / 6), step(n % 6))
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    }
}

pub fn hex(c: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

// ------------------------------------------------------------------ ansi ---

/// The SGR code for one of the sixteen named colours: 30–37 / 90–97 for the
/// foreground, +10 for the background. What a terminal that cannot do RGB
/// understands, and what the `ansi` theme promises to stay within.
fn named_sgr(c: Color, bg: bool) -> Option<u8> {
    let base = match c {
        Color::Black => 30,
        Color::Red => 31,
        Color::Green => 32,
        Color::Yellow => 33,
        Color::Blue => 34,
        Color::Magenta => 35,
        Color::Cyan => 36,
        Color::Gray => 37,
        Color::DarkGray => 90,
        Color::LightRed => 91,
        Color::LightGreen => 92,
        Color::LightYellow => 93,
        Color::LightBlue => 94,
        Color::LightMagenta => 95,
        Color::LightCyan => 96,
        Color::White => 97,
        _ => return None,
    };
    Some(if bg { base + 10 } else { base })
}

/// The escape sequence that selects `style`, resetting first. Honours the
/// theme's `truecolor` flag: a 16-colour theme emits the classic 3x/9x codes
/// rather than resolving its names to RGB, so it really does survive a
/// terminal, a multiplexer or a log file that cannot do 24-bit colour.
pub fn sgr(style: Style, t: &Theme) -> String {
    let mut parts: Vec<String> = vec!["0".into()];
    match style.fg {
        Some(Color::Reset) | None => {}
        Some(Color::Rgb(r, g, b)) => parts.push(format!("38;2;{r};{g};{b}")),
        Some(Color::Indexed(i)) => parts.push(format!("38;5;{i}")),
        Some(named) => match (t.truecolor, named_sgr(named, false)) {
            (false, Some(code)) => parts.push(code.to_string()),
            _ => {
                let (r, g, b) = to_rgb(named, t, false);
                parts.push(format!("38;2;{r};{g};{b}"));
            }
        },
    }
    match style.bg {
        Some(Color::Reset) | None => {}
        Some(Color::Rgb(r, g, b)) => parts.push(format!("48;2;{r};{g};{b}")),
        Some(Color::Indexed(i)) => parts.push(format!("48;5;{i}")),
        Some(named) => match (t.truecolor, named_sgr(named, true)) {
            (false, Some(code)) => parts.push(code.to_string()),
            _ => {
                let (r, g, b) = to_rgb(named, t, true);
                parts.push(format!("48;2;{r};{g};{b}"));
            }
        },
    }
    let m = style.add_modifier;
    if m.contains(Modifier::BOLD) {
        parts.push("1".into());
    }
    if m.contains(Modifier::DIM) {
        parts.push("2".into());
    }
    if m.contains(Modifier::ITALIC) {
        parts.push("3".into());
    }
    if m.contains(Modifier::UNDERLINED) {
        parts.push("4".into());
    }
    if m.contains(Modifier::REVERSED) {
        parts.push("7".into());
    }
    format!("\x1b[{}m", parts.join(";"))
}

fn same(a: &Style, b: &Style) -> bool {
    a.fg == b.fg && a.bg == b.bg && a.add_modifier == b.add_modifier
}

/// One row of cells, styled, with no line ending of any kind.
fn ansi_row(out: &mut String, buf: &Buffer, y: u16, t: &Theme) {
    let mut last: Option<Style> = None;
    for x in 0..buf.area.width {
        let cell = &buf[(x, y)];
        if cell.symbol().is_empty() {
            continue;
        }
        let style = cell.style();
        if last.as_ref().map(|l| !same(l, &style)).unwrap_or(true) {
            out.push_str(&sgr(style, t));
            last = Some(style);
        }
        out.push_str(cell.symbol());
    }
}

/// The buffer as newline-separated lines: for a file, a pipe, or a terminal
/// that is still in cooked mode. Use [`to_ansi_screen`] to paint a terminal
/// that is in raw mode.
pub fn to_ansi(buf: &Buffer, t: &Theme) -> String {
    let mut out = String::with_capacity(buf.area.area() as usize * 4);
    for y in 0..buf.area.height {
        ansi_row(&mut out, buf, y, t);
        out.push_str("\x1b[0m");
        out.push('\n');
    }
    out
}

/// The buffer as a frame painted onto a raw-mode terminal, starting at screen
/// row `top` (1-based) and column 1.
///
/// Every row is positioned absolutely instead of being reached with a newline.
/// Raw mode turns off `ONLCR`, so a bare `\n` is a line feed and nothing else:
/// it drops a row but keeps the column, which walks a full-width frame off the
/// right edge one row at a time. The trailing newline after the bottom row is
/// worse still — it scrolls the whole screen up. Positioning each row sidesteps
/// both, and the erase-to-end-of-line keeps a narrow row from leaving the
/// previous frame's tail behind it.
pub fn to_ansi_screen(buf: &Buffer, t: &Theme, top: u16) -> String {
    let mut out = String::with_capacity(buf.area.area() as usize * 4);
    for y in 0..buf.area.height {
        out.push_str(&format!("\x1b[{};1H", top.saturating_add(y)));
        ansi_row(&mut out, buf, y, t);
        // Erase first, so a terminal with background-colour erase clears in
        // the row's own colours rather than the default ground.
        out.push_str("\x1b[K\x1b[0m");
    }
    out
}

pub fn to_text(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            if cell.symbol().is_empty() {
                continue;
            }
            line.push_str(cell.symbol());
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

// ------------------------------------------------------------------ html ---

/// `s` with the five HTML/XML metacharacters escaped.
pub fn escaped(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    escape(s, &mut out);
    out
}

fn escape(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

/// A `<pre>` fragment. `class_prefix` namespaces nothing — every colour is
/// inline — so the fragment can be dropped into any page.
pub fn to_html_fragment(buf: &Buffer, t: &Theme) -> String {
    let bg = hex(to_rgb(t.pal.bg, t, true));
    let fg = hex(to_rgb(t.pal.fg, t, false));
    let mut out = String::with_capacity(buf.area.area() as usize * 12);
    out.push_str(&format!(
        "<pre class=\"pyxray\" style=\"background:{bg};color:{fg}\">"
    ));
    for y in 0..buf.area.height {
        let mut run_style: Option<Style> = None;
        let mut run = String::new();
        let flush = |run: &mut String, style: &Option<Style>, out: &mut String| {
            if run.is_empty() {
                return;
            }
            match style {
                Some(s) => {
                    let mut css = String::new();
                    if let Some(c) = s.fg {
                        if c != Color::Reset {
                            css.push_str(&format!("color:{};", hex(to_rgb(c, t, false))));
                        }
                    }
                    if let Some(c) = s.bg {
                        if c != Color::Reset {
                            css.push_str(&format!("background:{};", hex(to_rgb(c, t, true))));
                        }
                    }
                    if s.add_modifier.contains(Modifier::BOLD) {
                        css.push_str("font-weight:700;");
                    }
                    if s.add_modifier.contains(Modifier::DIM) {
                        css.push_str("opacity:.65;");
                    }
                    if s.add_modifier.contains(Modifier::ITALIC) {
                        css.push_str("font-style:italic;");
                    }
                    if s.add_modifier.contains(Modifier::UNDERLINED) {
                        css.push_str("text-decoration:underline;");
                    }
                    if css.is_empty() {
                        escape(run, out);
                    } else {
                        out.push_str(&format!("<span style=\"{css}\">"));
                        escape(run, out);
                        out.push_str("</span>");
                    }
                }
                None => escape(run, out),
            }
            run.clear();
        };

        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            if cell.symbol().is_empty() {
                continue;
            }
            let style = cell.style();
            if run_style.as_ref().map(|l| !same(l, &style)).unwrap_or(true) {
                flush(&mut run, &run_style, &mut out);
                run_style = Some(style);
            }
            run.push_str(cell.symbol());
        }
        flush(&mut run, &run_style, &mut out);
        out.push('\n');
    }
    out.push_str("</pre>");
    out
}

pub fn to_html(buf: &Buffer, t: &Theme, title: &str) -> String {
    let bg = hex(to_rgb(t.pal.bg, t, true));
    let fragment = to_html_fragment(buf, t);
    let title = escaped(title);
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\n<title>{title}</title>\n\
<style>\n  body {{ margin:0; padding:24px; background:{bg}; }}\n\
  pre.pyxray {{ font: 13px/1.32 \"JetBrains Mono\",\"Fira Code\",\"SF Mono\",\
\"DejaVu Sans Mono\",ui-monospace,monospace; margin:0; padding:16px; \
border-radius:6px; white-space:pre; overflow-x:auto; }}\n</style>\n</head>\n\
<body>\n{fragment}\n</body></html>\n"
    )
}

// ------------------------------------------------------------------- svg ---

const CELL_W: f32 = 8.4;
const CELL_H: f32 = 17.0;
const PAD: f32 = 14.0;

/// Vector output, one `<rect>` per background run and one `<text>` per
/// foreground run. Crisp at any size, and small enough to inline.
pub fn to_svg(buf: &Buffer, t: &Theme, title: &str) -> String {
    let w = buf.area.width as f32 * CELL_W + PAD * 2.0;
    let h = buf.area.height as f32 * CELL_H + PAD * 2.0;
    let bg = hex(to_rgb(t.pal.bg, t, true));
    let mut out = String::with_capacity(buf.area.area() as usize * 16);
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" \
width=\"{w:.0}\" height=\"{h:.0}\" font-family=\"JetBrains Mono, Fira Code, SF Mono, \
DejaVu Sans Mono, ui-monospace, monospace\" font-size=\"13\">\n"
    ));
    out.push_str(&format!("<title>{}</title>\n", escaped(title)));
    out.push_str(&format!(
        "<rect width=\"100%\" height=\"100%\" fill=\"{bg}\" rx=\"6\"/>\n"
    ));

    // Backgrounds first, merged into horizontal runs.
    for y in 0..buf.area.height {
        let mut run_start: Option<u16> = None;
        let mut run_colour: Option<Color> = None;
        for x in 0..=buf.area.width {
            let colour = if x == buf.area.width {
                None
            } else {
                match buf[(x, y)].style().bg {
                    Some(Color::Reset) | None => None,
                    Some(c) if c == t.pal.bg => None,
                    Some(c) => Some(c),
                }
            };
            if colour != run_colour {
                if let (Some(start), Some(c)) = (run_start, run_colour) {
                    let rx = PAD + start as f32 * CELL_W;
                    let rw = (x - start) as f32 * CELL_W;
                    let ry = PAD + y as f32 * CELL_H;
                    out.push_str(&format!(
                        "<rect x=\"{rx:.1}\" y=\"{ry:.1}\" width=\"{rw:.1}\" \
height=\"{CELL_H:.1}\" fill=\"{}\"/>\n",
                        hex(to_rgb(c, t, true))
                    ));
                }
                run_start = Some(x);
                run_colour = colour;
            }
        }
    }

    // Then text runs.
    for y in 0..buf.area.height {
        let baseline = PAD + y as f32 * CELL_H + CELL_H * 0.75;
        let mut run = String::new();
        let mut run_x: u16 = 0;
        let mut run_style: Option<Style> = None;
        let flush = |run: &mut String, style: &Option<Style>, rx: u16, out: &mut String| {
            if run.trim().is_empty() {
                run.clear();
                return;
            }
            let s = style.unwrap_or_default();
            let colour = hex(to_rgb(s.fg.unwrap_or(Color::Reset), t, false));
            let weight = if s.add_modifier.contains(Modifier::BOLD) {
                " font-weight=\"700\""
            } else {
                ""
            };
            let opacity = if s.add_modifier.contains(Modifier::DIM) {
                " opacity=\".65\""
            } else {
                ""
            };
            let x = PAD + rx as f32 * CELL_W;
            let mut esc = String::new();
            escape(run, &mut esc);
            out.push_str(&format!(
                "<text x=\"{x:.1}\" y=\"{baseline:.1}\" fill=\"{colour}\"{weight}{opacity} \
xml:space=\"preserve\">{esc}</text>\n"
            ));
            run.clear();
        };

        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            if cell.symbol().is_empty() {
                continue;
            }
            let style = cell.style();
            if run_style.as_ref().map(|l| !same(l, &style)).unwrap_or(true) {
                flush(&mut run, &run_style, run_x, &mut out);
                run_style = Some(style);
                run_x = x;
            }
            run.push_str(cell.symbol());
        }
        flush(&mut run, &run_style, run_x, &mut out);
    }

    out.push_str("</svg>\n");
    out
}

pub fn emit(buf: &Buffer, t: &Theme, f: Format, title: &str) -> String {
    match f {
        Format::Ansi => to_ansi(buf, t),
        Format::Text => to_text(buf),
        Format::Html => to_html(buf, t, title),
        Format::Svg => to_svg(buf, t, title),
    }
}
