//! Drawing primitives over a ratatui [`Buffer`]. Everything the panels draw
//! goes through these, so clipping and character-width handling live in one
//! place and the exporters see a buffer that is always well-formed.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use crate::theme::{Frame, Theme};

pub type Seg<'a> = (&'a str, Style);

pub fn width(s: &str) -> u16 {
    UnicodeWidthStr::width(s).min(u16::MAX as usize) as u16
}

/// Truncate to a display width, appending `ellipsis` when anything was cut.
pub fn fit(s: &str, max: u16, ellipsis: char) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let budget = max.saturating_sub(1) as usize;
    let mut out = String::new();
    let mut used = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthStr::width(ch.encode_utf8(&mut [0u8; 4]) as &str);
        if used + w > budget {
            break;
        }
        used += w;
        out.push(ch);
    }
    out.push(ellipsis);
    out
}

pub fn fill(buf: &mut Buffer, area: Rect, style: Style) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ');
                cell.set_style(style);
            }
        }
    }
}

/// Write a string, clipped to `max_w`. Returns the width consumed.
pub fn text(buf: &mut Buffer, x: u16, y: u16, max_w: u16, s: &str, style: Style) -> u16 {
    if max_w == 0 || y >= buf.area.bottom() || x >= buf.area.right() {
        return 0;
    }
    let room = max_w.min(buf.area.right().saturating_sub(x));
    let mut cx = x;
    for ch in s.chars() {
        let mut tmp = [0u8; 4];
        let piece = ch.encode_utf8(&mut tmp);
        let w = width(piece).max(1);
        if cx + w > x + room {
            break;
        }
        if let Some(cell) = buf.cell_mut((cx, y)) {
            cell.set_symbol(piece);
            cell.set_style(style);
        }
        // A wide glyph owns the cell to its right; blank it so the terminal
        // and the exporters agree about where the next character starts.
        for pad in 1..w {
            if let Some(cell) = buf.cell_mut((cx + pad, y)) {
                cell.set_symbol("");
                cell.set_style(style);
            }
        }
        cx += w;
    }
    cx - x
}

/// Write a run of styled segments left to right, stopping at `max_w`.
pub fn segs(buf: &mut Buffer, x: u16, y: u16, max_w: u16, parts: &[Seg]) -> u16 {
    let mut cx = x;
    for (s, style) in parts {
        if cx >= x + max_w {
            break;
        }
        cx += text(buf, cx, y, x + max_w - cx, s, *style);
    }
    cx - x
}

/// Write segments so that they *end* at `right_edge`, refusing to start before
/// `left_bound`. Returns where the run began, or `right_edge` if it did not
/// fit at all — a right-aligned run that overflows would otherwise walk over
/// whatever is to its left, frame included.
pub fn segs_right_bounded(
    buf: &mut Buffer,
    left_bound: u16,
    right_edge: u16,
    y: u16,
    parts: &[Seg],
) -> u16 {
    let total: u16 = parts.iter().map(|(s, _)| width(s)).sum();
    if total == 0 || right_edge.saturating_sub(left_bound) < total {
        return right_edge;
    }
    let x = right_edge - total;
    segs(buf, x, y, total, parts);
    x
}

/// As [`segs_right_bounded`], anchored at the left edge of the buffer.
pub fn segs_right(buf: &mut Buffer, right_edge: u16, y: u16, parts: &[Seg]) -> u16 {
    segs_right_bounded(buf, 0, right_edge, y, parts)
}

pub fn hline(buf: &mut Buffer, x: u16, y: u16, w: u16, ch: char, style: Style) {
    let s: String = std::iter::repeat_n(ch, w as usize).collect();
    text(buf, x, y, w, &s, style);
}

pub fn vline(buf: &mut Buffer, x: u16, y: u16, h: u16, ch: char, style: Style) {
    let mut tmp = [0u8; 4];
    let piece = ch.encode_utf8(&mut tmp).to_string();
    for yy in y..y + h {
        text(buf, x, yy, 1, &piece, style);
    }
}

/// Draw a panel frame in the theme's style and return the area left for
/// content. `title` may be empty.
pub fn frame(buf: &mut Buffer, area: Rect, theme: &Theme, title: &str) -> Rect {
    if area.width < 2 || area.height < 1 {
        return area;
    }
    let g = theme.gl;
    let rs = theme.rule_style();
    let ts = theme.title();
    let head = if title.is_empty() {
        String::new()
    } else {
        theme.head(title)
    };

    match theme.frame {
        Frame::None => {
            if head.is_empty() {
                area
            } else {
                text(buf, area.x, area.y, area.width, &head, ts);
                shrink_top(area, 1)
            }
        }
        Frame::Underline => {
            let mut y = area.y;
            if !head.is_empty() {
                text(buf, area.x, y, area.width, &head, ts);
                y += 1;
            }
            hline(buf, area.x, y, area.width, g.h, rs);
            shrink_top(area, y + 1 - area.y)
        }
        Frame::Bracket => {
            let w = area.width;
            let h = area.height;
            let corner = |buf: &mut Buffer, x: u16, y: u16, s: &str| {
                text(buf, x, y, 2, s, rs);
            };
            corner(buf, area.x, area.y, "\u{250c}");
            corner(buf, area.x + w - 1, area.y, "\u{2510}");
            if h >= 2 {
                corner(buf, area.x, area.y + h - 1, "\u{2514}");
                corner(buf, area.x + w - 1, area.y + h - 1, "\u{2518}");
            }
            if !head.is_empty() {
                text(buf, area.x + 2, area.y, w.saturating_sub(4), &head, ts);
            }
            Rect {
                x: area.x + 2,
                y: area.y + 1,
                width: w.saturating_sub(4),
                height: h.saturating_sub(2),
            }
        }
        _ => {
            let w = area.width;
            let h = area.height.max(2);
            // top
            let mut tx = area.x;
            text(buf, tx, area.y, 1, &g.tl.to_string(), rs);
            tx += 1;
            if head.is_empty() {
                hline(buf, tx, area.y, w.saturating_sub(2), g.h, rs);
            } else {
                text(buf, tx, area.y, 1, &g.h.to_string(), rs);
                tx += 1;
                text(buf, tx, area.y, 1, " ", rs);
                tx += 1;
                let room = w.saturating_sub(6);
                let cut = fit(&head, room, theme.gl.ellipsis);
                tx += text(buf, tx, area.y, room, &cut, ts);
                tx += text(buf, tx, area.y, 1, " ", rs);
                let rest = (area.x + w).saturating_sub(tx + 1);
                hline(buf, tx, area.y, rest, g.h, rs);
            }
            text(buf, area.x + w - 1, area.y, 1, &g.tr.to_string(), rs);
            // sides
            vline(buf, area.x, area.y + 1, h.saturating_sub(2), g.v, rs);
            vline(
                buf,
                area.x + w - 1,
                area.y + 1,
                h.saturating_sub(2),
                g.v,
                rs,
            );
            // bottom
            text(buf, area.x, area.y + h - 1, 1, &g.bl.to_string(), rs);
            hline(
                buf,
                area.x + 1,
                area.y + h - 1,
                w.saturating_sub(2),
                g.h,
                rs,
            );
            text(
                buf,
                area.x + w - 1,
                area.y + h - 1,
                1,
                &g.br.to_string(),
                rs,
            );
            Rect {
                x: area.x + 2,
                y: area.y + 1,
                width: w.saturating_sub(4),
                height: h.saturating_sub(2),
            }
        }
    }
}

/// Rows of chrome a frame costs, so layouts can budget before drawing.
pub fn frame_overhead(theme: &Theme, has_title: bool) -> (u16, u16) {
    match theme.frame {
        Frame::None => (if has_title { 1 } else { 0 }, 0),
        Frame::Underline => (if has_title { 2 } else { 1 }, 0),
        Frame::Bracket => (1, 4),
        _ => (2, 4),
    }
}

pub fn shrink_top(area: Rect, rows: u16) -> Rect {
    Rect {
        x: area.x,
        y: area.y + rows,
        width: area.width,
        height: area.height.saturating_sub(rows),
    }
}

/// A horizontal meter. `frac` is 0..=1; the track is always drawn so the
/// reader can see the scale even at zero.
#[allow(clippy::too_many_arguments)]
pub fn meter(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    w: u16,
    frac: f32,
    theme: &Theme,
    on: Style,
    off: Style,
) {
    if w == 0 {
        return;
    }
    let filled = (frac.clamp(0.0, 1.0) * w as f32).round() as u16;
    let full = theme.gl.ramp[theme.gl.ramp.len() - 1];
    let track = if theme.gl.ramp[0] == '\u{00b7}' {
        '\u{2591}'
    } else {
        theme.gl.ramp[0]
    };
    for i in 0..w {
        let (ch, st) = if i < filled { (full, on) } else { (track, off) };
        text(buf, x + i, y, 1, &ch.to_string(), st);
    }
}

/// A pill-shaped label. Returns the width consumed.
pub fn chip(buf: &mut Buffer, x: u16, y: u16, max_w: u16, label: &str, style: Style) -> u16 {
    let body = format!(" {label} ");
    text(buf, x, y, max_w, &body, style)
}
