//! The contact sheet: every theme and every layout for one snippet, on one
//! page, so a look can be chosen by comparison rather than by imagination.

use pyxray_core::model::Report;
use pyxray_render::export;
use pyxray_render::layout::{self, Layout};
use pyxray_render::panels::{Icons, Opts};
use pyxray_render::theme::{Theme, THEMES};

fn cell(report: &Report, theme: &Theme, l: Layout, opts: &Opts, width: u16) -> String {
    let buf = layout::render(report, theme, opts, l, width, None);
    export::to_html_fragment(&buf, theme)
}

pub fn build(report: &Report, width: u16, base: &Opts) -> String {
    let mut out = String::new();
    out.push_str(HEAD);

    out.push_str(&format!(
        "<header><h1>pyxray contact sheet</h1><p class=\"sub\">{} \u{00b7} {} lines \u{00b7} \
risk {}</p><p class=\"syn\">{}</p></header>\n",
        html_escape(&report.meta.name),
        report.metrics.lines_total,
        report.metrics.risk,
        html_escape(&report.meta.synopsis)
    ));

    // Axis one: the same layout in every theme.
    out.push_str("<section><h2>Themes</h2><p class=\"note\">Layout held at <code>dashboard</code>, icons at <code>both</code>. Pick the ground and the accent language.</p><div class=\"grid\">\n");
    for theme in THEMES {
        out.push_str(&format!(
            "<figure class=\"card\"><figcaption><b>{}</b><span>{}</span></figcaption>{}</figure>\n",
            theme.id,
            html_escape(theme.blurb),
            cell(report, theme, Layout::Dashboard, base, width)
        ));
    }
    out.push_str("</div></section>\n");

    // Axis two: the same theme in every layout.
    let pick = THEMES[0];
    out.push_str(&format!(
        "<section><h2>Layouts</h2><p class=\"note\">Theme held at <code>{}</code>. Pick how much you want to see at once.</p><div class=\"grid\">\n",
        pick.id
    ));
    for l in Layout::all() {
        out.push_str(&format!(
            "<figure class=\"card\"><figcaption><b>{}</b><span>{}</span></figcaption>{}</figure>\n",
            l.id(),
            html_escape(l.blurb()),
            cell(report, &pick, l, base, width)
        ));
    }
    out.push_str("</div></section>\n");

    // Axis three: how effects are labelled.
    out.push_str("<section><h2>Effect labelling</h2><p class=\"note\">Glyphs are dense and quiet; words are unmissable and wide. Both is the default.</p><div class=\"grid\">\n");
    for (mode, name, why) in [
        (
            Icons::Glyph,
            "glyph",
            "One character per capability. Densest, needs the legend learned once.",
        ),
        (
            Icons::Tag,
            "tag",
            "Words only. Nothing to learn, costs horizontal room.",
        ),
        (
            Icons::Both,
            "both",
            "Glyph plus word. Reads instantly, widest.",
        ),
    ] {
        let opts = Opts {
            icons: mode,
            ..*base
        };
        out.push_str(&format!(
            "<figure class=\"card\"><figcaption><b>{name}</b><span>{}</span></figcaption>{}</figure>\n",
            html_escape(why),
            cell(report, &pick, Layout::Card, &opts, width)
        ));
    }
    out.push_str("</div></section>\n");

    out.push_str("</main></body></html>\n");
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const HEAD: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>pyxray contact sheet</title>
<style>
  :root { color-scheme: dark; --ink:#e8e6e3; --dim:#9a958e; --ground:#101014; --edge:#2a2a32; }
  * { box-sizing: border-box; }
  body { margin:0; background:var(--ground); color:var(--ink);
         font:15px/1.5 ui-sans-serif,-apple-system,"Segoe UI",Roboto,sans-serif; }
  main, header { max-width: 1500px; margin: 0 auto; padding: 0 28px; }
  header { padding-top: 48px; padding-bottom: 8px; }
  h1 { font-size: 30px; margin:0 0 6px; letter-spacing:-0.02em; }
  h2 { font-size: 15px; text-transform: uppercase; letter-spacing:.14em;
       color: var(--dim); margin: 44px 0 4px; font-weight:600; }
  .sub { color: var(--dim); margin:0 0 10px; font-variant-numeric: tabular-nums; }
  .syn { font-size:17px; margin:0 0 8px; }
  .note { color: var(--dim); margin: 0 0 18px; max-width: 70ch; }
  code { background:#1c1c22; padding:1px 5px; border-radius:4px; font-size:.9em; }
  .grid { display:grid; gap:22px; grid-template-columns: 1fr; }
  .card { margin:0; border:1px solid var(--edge); border-radius:10px; overflow:hidden; background:#16161b; }
  figcaption { display:flex; gap:12px; align-items:baseline; flex-wrap:wrap;
               padding:10px 14px; border-bottom:1px solid var(--edge); background:#1a1a20; }
  figcaption b { font-size:13px; letter-spacing:.08em; text-transform:uppercase; }
  figcaption span { color:var(--dim); font-size:13px; }
  pre.pyxray { font: 12.5px/1.3 "JetBrains Mono","Fira Code","SF Mono","DejaVu Sans Mono",ui-monospace,monospace;
               margin:0; padding:16px; white-space:pre; overflow-x:auto; }
  @media (min-width: 1100px) { .grid { grid-template-columns: 1fr; } }
</style></head><body><main>
"#;
