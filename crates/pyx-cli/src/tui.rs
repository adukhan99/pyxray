//! The interactive viewer. Its real job is lookdev: every visual decision is
//! on a key, so a look gets chosen by flipping through candidates against real
//! code rather than by reading a description of it.

use std::io::{self, Stdout, Write};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, terminal};
use pyxray_core::model::Report;
use pyxray_render::export;
use pyxray_render::layout::{self, Layout};
use pyxray_render::panels::Icons;
use pyxray_render::theme::THEMES;

use crate::args::Args;

struct State {
    theme: usize,
    layout: usize,
    icons: Icons,
    depth: u16,
    code: bool,
    gutter: bool,
    scroll: u16,
    status: String,
}

pub fn run(report: &Report, a: &Args) -> io::Result<()> {
    let mut state = State {
        theme: THEMES.iter().position(|t| t.id == a.theme.id).unwrap_or(0),
        layout: Layout::all()
            .iter()
            .position(|l| *l == a.layout)
            .unwrap_or(0),
        icons: a.opts.icons,
        depth: a.opts.max_depth,
        code: a.opts.code,
        gutter: a.opts.gutter,
        scroll: 0,
        status: "t theme · l layout · i icons · d depth · c code · s save · q quit".into(),
    };

    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
    let result = event_loop(&mut out, report, &mut state);
    execute!(out, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    result
}

fn event_loop(out: &mut Stdout, report: &Report, state: &mut State) -> io::Result<()> {
    loop {
        draw(out, report, state)?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let shift = modifiers.contains(KeyModifiers::SHIFT);
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(())
                    }
                    KeyCode::Char('t') => {
                        state.theme = step(state.theme, THEMES.len(), shift);
                        state.status = format!("theme: {}", THEMES[state.theme].name);
                    }
                    KeyCode::Char('l') => {
                        state.layout = step(state.layout, Layout::all().len(), shift);
                        state.status = format!("layout: {}", Layout::all()[state.layout].id());
                    }
                    KeyCode::Char('i') => {
                        state.icons = match state.icons {
                            Icons::Glyph => Icons::Tag,
                            Icons::Tag => Icons::Both,
                            Icons::Both => Icons::Glyph,
                        };
                        state.status = format!("icons: {:?}", state.icons);
                    }
                    KeyCode::Char('c') => {
                        state.code = !state.code;
                        state.status = format!("source: {}", on_off(state.code));
                    }
                    KeyCode::Char('g') => {
                        state.gutter = !state.gutter;
                        state.status = format!("line numbers: {}", on_off(state.gutter));
                    }
                    KeyCode::Char('d') => {
                        state.depth = state.depth.saturating_add(1).min(24);
                        state.status = format!("depth: {}", state.depth);
                    }
                    KeyCode::Char('D') => {
                        state.depth = state.depth.saturating_sub(1).max(1);
                        state.status = format!("depth: {}", state.depth);
                    }
                    KeyCode::Char('s') => state.status = save(report, state),
                    KeyCode::Down | KeyCode::Char('j') => {
                        state.scroll = state.scroll.saturating_add(1)
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.scroll = state.scroll.saturating_sub(1)
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        state.scroll = state.scroll.saturating_add(10)
                    }
                    KeyCode::PageUp => state.scroll = state.scroll.saturating_sub(10),
                    KeyCode::Home => state.scroll = 0,
                    _ => {}
                }
            }
            Event::Resize(_, _) => state.scroll = 0,
            _ => {}
        }
    }
}

fn step(current: usize, len: usize, back: bool) -> usize {
    if back {
        (current + len - 1) % len
    } else {
        (current + 1) % len
    }
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

fn opts(state: &State) -> pyxray_render::Opts {
    pyxray_render::Opts {
        max_depth: state.depth,
        gutter: state.gutter,
        icons: state.icons,
        code: state.code,
    }
}

fn draw(out: &mut Stdout, report: &Report, state: &State) -> io::Result<()> {
    let (w, h) = terminal::size()?;
    let theme = THEMES[state.theme];
    let l = Layout::all()[state.layout];
    let body_h = h.saturating_sub(1).max(3);

    // Render tall, then window into it — that keeps scrolling honest even for
    // layouts that would rather be longer than the terminal.
    let buf = layout::render(report, &theme, &opts(state), l, w, None);
    let total = buf.area.height;
    let max_scroll = total.saturating_sub(body_h);
    let top = state.scroll.min(max_scroll);

    let mut screen = String::with_capacity((w as usize) * (h as usize) * 4);
    screen.push_str("\x1b[H");
    let window = crop(&buf, top, body_h);
    screen.push_str(&export::to_ansi(&window, &theme));

    // Status line, in the theme's own colours so it never fights the render.
    let (br, bg, bb) = export::to_rgb(theme.pal.rule, &theme, true);
    let (fr, fg, fb) = export::to_rgb(theme.pal.fg, &theme, false);
    let left = format!(
        " {} · {} · {:?} · depth {} ",
        theme.name,
        l.id(),
        state.icons,
        state.depth
    );
    let right = if total > body_h {
        format!(" {}/{} ", top + body_h.min(total), total)
    } else {
        String::new()
    };
    let pad = (w as usize).saturating_sub(
        left.chars().count() + right.chars().count() + state.status.chars().count() + 3,
    );
    screen.push_str(&format!(
        "\x1b[{};1H\x1b[48;2;{br};{bg};{bb}m\x1b[38;2;{fr};{fg};{fb}m{left}\x1b[2m {} \x1b[22m{}{right}\x1b[0m",
        h,
        state.status,
        " ".repeat(pad)
    ));
    out.write_all(screen.as_bytes())?;
    out.flush()
}

/// Take `rows` rows starting at `top` out of a taller buffer.
fn crop(buf: &ratatui::buffer::Buffer, top: u16, rows: u16) -> ratatui::buffer::Buffer {
    let area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: buf.area.width,
        height: rows.min(buf.area.height.saturating_sub(top)).max(1),
    };
    let mut out = ratatui::buffer::Buffer::empty(area);
    for y in 0..area.height {
        for x in 0..area.width {
            if let Some(src) = buf.cell((x, y + top)) {
                if let Some(dst) = out.cell_mut((x, y)) {
                    *dst = src.clone();
                }
            }
        }
    }
    out
}

fn save(report: &Report, state: &State) -> String {
    let theme = THEMES[state.theme];
    let l = Layout::all()[state.layout];
    let buf = layout::render(report, &theme, &opts(state), l, 120, None);
    let stem = format!("pyxray-{}-{}", theme.id, l.id());
    let html = export::to_html(&buf, &theme, &report.meta.name);
    let svg = export::to_svg(&buf, &theme, &report.meta.name);
    match (
        std::fs::write(format!("{stem}.html"), html),
        std::fs::write(format!("{stem}.svg"), svg),
    ) {
        (Ok(()), Ok(())) => format!("saved {stem}.html and {stem}.svg"),
        _ => "could not write files here".to_string(),
    }
}
