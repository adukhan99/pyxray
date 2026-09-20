//! What the renders actually look like, pinned. `cargo insta review` shows a
//! change as a diff of the drawing, which is the only sensible way to review
//! a change to a layout.

use pyxray_core::xray;
use pyxray_render::export;
use pyxray_render::layout::{self, Layout};
use pyxray_render::panels::Opts;
use pyxray_render::theme::THEMES;

fn sketchy() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sketchy.py");
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

#[test]
fn every_theme_and_gallery_layout_at_100_columns() {
    let report = xray(&sketchy(), "sketchy.py");
    for theme in THEMES {
        for l in Layout::gallery() {
            let buf = layout::render(&report, theme, &Opts::default(), l, 100, None);
            let text = export::to_text(&buf);
            insta::assert_snapshot!(format!("{}-{}", theme.id, l.id()), text);
        }
    }
}

#[test]
fn the_feed_rows_and_html_export() {
    let report = xray(&sketchy(), "sketchy.py");
    let theme = THEMES[0];
    let buf = layout::render(&report, &theme, &Opts::default(), Layout::Card, 100, None);
    insta::assert_snapshot!(
        "blueprint-card-html",
        export::to_html(&buf, &theme, "sketchy.py")
    );
    let ansi = export::to_ansi(&buf, &theme);
    insta::assert_snapshot!("blueprint-card-ansi", ansi);
}
