//! `pyx watch` — a live column of everything Python that went past.
//!
//! This is the answer to a harness firing several snippets a second. Drawing a
//! report per snippet into the terminal the agent is talking through is
//! unreadable and, in most harnesses, feeds straight back to the model as
//! tool output. So the renders come here instead: a second pane, its own
//! window, a tmux split — somewhere a person is watching and the model is not.

use std::collections::VecDeque;
use std::io::{self, IsTerminal, Stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event as TermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, terminal};
use pyxray_core::feed::{self, Tail};
use pyxray_core::model::{Band, Event};
use pyxray_render::export;
use pyxray_render::feed as feedview;
use pyxray_render::panels::Opts;
use pyxray_render::theme::{Theme, THEMES};

/// How many events to keep on screen. Older ones fall off the top; the log on
/// disk keeps them.
const BACKLOG: usize = 2000;

pub struct Options {
    pub path: PathBuf,
    pub theme: Theme,
    pub opts: Opts,
    /// Start from the whole log rather than only what arrives from now on.
    pub replay: bool,
    /// Hide anything below this band.
    pub floor: Band,
    /// Render what is in the log once and exit, instead of following it.
    pub dump: bool,
    /// Columns and rows to render at when dumping; `None` asks the terminal.
    pub size: (Option<u16>, Option<u16>),
}

struct State {
    events: VecDeque<Event>,
    stats: feedview::Stats,
    theme: usize,
    floor: Band,
    paused: bool,
    /// Rows scrolled up from the live edge; zero means following.
    scroll: usize,
    notes: bool,
    status: String,
}

pub fn run(o: Options) -> io::Result<()> {
    let mut state = State {
        events: VecDeque::new(),
        stats: feedview::Stats::default(),
        theme: THEMES.iter().position(|t| t.id == o.theme.id).unwrap_or(0),
        floor: o.floor,
        paused: false,
        scroll: 0,
        notes: true,
        status: String::new(),
    };

    state.status =
        "q quit \u{00b7} p pause \u{00b7} f filter \u{00b7} n notes \u{00b7} t theme \u{00b7} c clear"
            .to_string();

    if o.dump {
        for event in feed::read_all(&o.path).unwrap_or_default() {
            state.stats.push(&event);
            state.events.push_back(event);
        }
        while state.events.len() > BACKLOG {
            state.events.pop_front();
        }
        let (tw, th) = terminal::size().unwrap_or((100, 40));
        let w = o.size.0.unwrap_or(tw).max(40);
        let h = o.size.1.unwrap_or_else(|| {
            // Tall enough for everything, so a dump is never silently cut.
            (state.events.len() as u16).saturating_add(6).max(th)
        });
        let buf = compose(&state, &o, w, h);
        let text = if std::io::stdout().is_terminal() {
            export::to_ansi(&buf, &THEMES[state.theme])
        } else {
            export::to_text(&buf)
        };
        print!("{text}");
        return Ok(());
    }

    let mut tail = if o.replay {
        Tail::at_start(&o.path)
    } else {
        // Seed the totals from the log so the header is not a lie, then follow
        // only what arrives next.
        for event in feed::read_all(&o.path).unwrap_or_default() {
            state.stats.push(&event);
        }
        Tail::at_end(&o.path)
    };

    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
    let result = pump(&mut out, &mut tail, &mut state, &o);
    execute!(out, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    result
}

fn pump(out: &mut Stdout, tail: &mut Tail, state: &mut State, o: &Options) -> io::Result<()> {
    loop {
        for event in tail.poll() {
            state.stats.push(&event);
            if !state.paused {
                state.events.push_back(event);
                while state.events.len() > BACKLOG {
                    state.events.pop_front();
                }
            }
        }
        draw(out, state, o)?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        match event::read()? {
            TermEvent::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) => match code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                KeyCode::Char('p') => {
                    state.paused = !state.paused;
                    state.status = if state.paused {
                        "paused \u{00b7} p to resume".into()
                    } else {
                        "following".into()
                    };
                }
                KeyCode::Char('f') => {
                    state.floor = match state.floor {
                        Band::Inert => Band::Routine,
                        Band::Routine => Band::Check,
                        Band::Check => Band::Read,
                        Band::Read => Band::Inert,
                    };
                    state.status = format!("showing {} and above", state.floor.word());
                }
                KeyCode::Char('n') => {
                    state.notes = !state.notes;
                    state.status = format!("notes {}", if state.notes { "on" } else { "off" });
                }
                KeyCode::Char('t') => {
                    state.theme = (state.theme + 1) % THEMES.len();
                    state.status = format!("theme: {}", THEMES[state.theme].name);
                }
                KeyCode::Char('c') => {
                    state.events.clear();
                    state.scroll = 0;
                    state.status = "cleared the view; the log is untouched".into();
                }
                KeyCode::Up | KeyCode::Char('k') => state.scroll += 1,
                KeyCode::Down | KeyCode::Char('j') => state.scroll = state.scroll.saturating_sub(1),
                KeyCode::PageUp => state.scroll += 10,
                KeyCode::PageDown => state.scroll = state.scroll.saturating_sub(10),
                KeyCode::End => state.scroll = 0,
                _ => {}
            },
            TermEvent::Resize(_, _) => state.scroll = 0,
            _ => {}
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn draw(out: &mut Stdout, state: &State, o: &Options) -> io::Result<()> {
    let (w, h) = terminal::size()?;
    let buf = compose(state, o, w, h);
    let theme = THEMES[state.theme];
    let mut screen = String::with_capacity((w as usize) * (h as usize) * 4);
    screen.push_str("\x1b[H");
    screen.push_str(&export::to_ansi(&buf, &theme));
    out.write_all(screen.as_bytes())?;
    out.flush()
}

fn compose(state: &State, o: &Options, w: u16, h: u16) -> ratatui::buffer::Buffer {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let theme = THEMES[state.theme];
    let area = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    let mut buf = Buffer::empty(area);
    pyxray_render::canvas::fill(&mut buf, area, theme.base());

    let head_h = 3u16.min(h);
    feedview::header(
        &mut buf,
        Rect {
            x: 1,
            y: 0,
            width: w.saturating_sub(2),
            height: head_h,
        },
        &state.stats,
        &o.path.display().to_string(),
        &theme,
    );
    pyxray_render::canvas::hline(&mut buf, 0, head_h, w, theme.gl.h, theme.rule_style());

    let body_top = head_h + 1;
    let body_h = h.saturating_sub(body_top + 1);

    // Build the rows newest-last, honouring the filter, then window them.
    let visible: Vec<&Event> = state
        .events
        .iter()
        .filter(|e| e.band >= state.floor)
        .collect();

    let mut rows: Vec<(&Event, bool)> = Vec::new();
    for event in &visible {
        rows.push((event, false));
        if state.notes
            && event.band >= Band::Check
            && event
                .notes
                .first()
                .is_some_and(|n| n.severity >= pyxray_core::model::Severity::Caution)
        {
            rows.push((event, true));
        }
    }

    let total = rows.len();
    let window = body_h as usize;
    let end = total.saturating_sub(state.scroll);
    let start = end.saturating_sub(window);
    let now = now_ms();

    for (i, (event, is_note)) in rows[start..end].iter().enumerate() {
        let y = body_top + i as u16;
        let line = Rect {
            x: 1,
            y,
            width: w.saturating_sub(2),
            height: 1,
        };
        if *is_note {
            feedview::note_row(&mut buf, line, event, &theme);
        } else {
            feedview::row(&mut buf, line, event, now, &theme, &o.opts);
        }
    }

    if total == 0 {
        let hint = if state.stats.count == 0 {
            "nothing yet \u{2014} run some Python through a shim or a hook and it will appear here"
        } else {
            "nothing above the current filter \u{2014} press f to widen it"
        };
        pyxray_render::canvas::text(
            &mut buf,
            2,
            body_top + 1,
            w.saturating_sub(4),
            hint,
            theme.faint(),
        );
    }

    // Status line.
    let y = h.saturating_sub(1);
    let mode = if state.paused {
        "paused"
    } else if state.scroll > 0 {
        "scrolled"
    } else {
        "live"
    };
    let left = format!(
        " {mode} \u{00b7} {} shown \u{00b7} {} and above ",
        total,
        state.floor.word()
    );
    let bar = theme.base().bg(theme.pal.rule).fg(theme.pal.fg);
    pyxray_render::canvas::fill(
        &mut buf,
        Rect {
            x: 0,
            y,
            width: w,
            height: 1,
        },
        bar,
    );
    let used = pyxray_render::canvas::text(&mut buf, 0, y, w, &left, bar);
    pyxray_render::canvas::text(
        &mut buf,
        used,
        y,
        w.saturating_sub(used),
        &format!(" {}", state.status),
        bar.fg(theme.pal.dim),
    );

    buf
}
