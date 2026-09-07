"""Assemble the lookdev contact sheet from rendered fragments.

Usage: python3 tools/render_fragments.py && python3 tools/build_lookdev.py
"""

import json, pathlib, html, sys
S = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "build/lookdev")
F = json.loads((S / "frags.json").read_text())
I = json.loads((S / "themeinfo.json").read_text())
THEMES = ["blueprint", "neon", "paper", "carbon", "amber", "ansi", "mono"]
LAYOUTS = [
    ("card", "A glance. Synopsis, risk meter, capability chips, effect timeline. Sized to sit in front of a command you are about to run.", "96 cols"),
    ("dashboard", "Two columns. The outline on the left, the findings on the right. Falls back to stack below 96 columns.", "118 cols"),
    ("stack", "One column, everything, in reading order. Imports, values and calls included.", "96 cols"),
    ("flow", "The outline given almost all the room, with a short timeline underneath.", "96 cols"),
]
ICONS = [
    ("glyph", "One character per capability.", "Densest. Costs the reader one look at the legend, once."),
    ("tag", "Words only, no glyphs.", "Nothing to learn. Costs the most horizontal room."),
    ("both", "Glyph then word.", "Reads instantly. Widest, and the current default."),
]

def specimen(frag):
    return f'<div class="render">{frag}</div>'

def band(kind, rows):
    out = []
    for r in rows:
        out.append(f'''<article class="frame" data-kind="{kind}" data-value="{r["id"]}">
  <div class="caption">
    <div class="capline">
      <h3>{html.escape(r["title"])}</h3>
      <code class="flag">{html.escape(r["flag"])}</code>
    </div>
    <p>{html.escape(r["blurb"])}</p>
    {r.get("meta","")}
    <button class="mark" type="button" aria-pressed="false">
      <span class="ring" aria-hidden="true"></span><span class="marktext">Mark</span>
    </button>
  </div>
  <div class="plate">{r["body"]}</div>
</article>''')
    return "\n".join(out)

theme_rows = []
for t in THEMES:
    info = I[t]
    if info["body"] is None:
        meta = ('<dl class="stats"><div><dt>contrast</dt><dd>set by your terminal</dd></div>'
                '<div><dt>ground</dt><dd>inherited</dd></div></dl>')
    else:
        meta = (f'<dl class="stats"><div><dt>body text</dt><dd>{info["body"]}:1</dd></div>'
                f'<div><dt>weakest role</dt><dd>{info["worst"]}:1</dd></div>'
                f'<div><dt>ground</dt><dd><span class="swatch" style="background:{info["bg"]}"></span>{info["bg"]}</dd></div></dl>')
    body = (specimen(F["themeWide"][t]).replace('class="render"', 'class="render" data-w="wide"')
            + specimen(F["themeNarrow"][t]).replace('class="render"', 'class="render" data-w="narrow" hidden'))
    theme_rows.append({"id": t, "title": t, "flag": f"-t {t}",
                       "blurb": info["blurb"], "meta": meta, "body": body})

layout_rows = [{"id": l, "title": l, "flag": f"-l {l}", "blurb": b,
                "meta": f'<dl class="stats"><div><dt>rendered at</dt><dd>{w}</dd></div></dl>',
                "body": specimen(F["layouts"][l])} for l, b, w in LAYOUTS]

icon_rows = [{"id": i, "title": i, "flag": f"-i {i}", "blurb": f"{b} {why}",
              "body": specimen(F["icons"][i])} for i, b, why in ICONS]

PAGE = """<title>pyxray Contact Sheet</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Archivo:wght@500;600;700&family=IBM+Plex+Mono:wght@400;500&family=IBM+Plex+Sans:wght@400;500&family=JetBrains+Mono:wght@400;700&display=swap">
<style>
:root {
  --ground:#f4f4f2; --panel:#ffffff; --sunk:#ececeb;
  --ink:#15171c; --muted:#5b6170; --faint:#8b909c;
  --line:#dedfe2; --line-soft:#e9e9ec;
  --mark:#c8341f;
  --shadow:0 1px 2px rgba(20,22,28,.06), 0 6px 18px rgba(20,22,28,.05);
  color-scheme:light dark;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --ground:#131519; --panel:#1a1d23; --sunk:#101216;
    --ink:#e9eaee; --muted:#959bab; --faint:#6f7583;
    --line:#282c35; --line-soft:#21242c;
    --mark:#e8604a;
    --shadow:0 1px 2px rgba(0,0,0,.4), 0 8px 24px rgba(0,0,0,.28);
  }
}
:root[data-theme="dark"] {
  --ground:#131519; --panel:#1a1d23; --sunk:#101216;
  --ink:#e9eaee; --muted:#959bab; --faint:#6f7583;
  --line:#282c35; --line-soft:#21242c;
  --mark:#e8604a;
  --shadow:0 1px 2px rgba(0,0,0,.4), 0 8px 24px rgba(0,0,0,.28);
}

* { box-sizing:border-box; }
body {
  margin:0; background:var(--ground); color:var(--ink);
  font:16px/1.55 "IBM Plex Sans", ui-sans-serif, -apple-system, "Segoe UI", Roboto, sans-serif;
  -webkit-font-smoothing:antialiased;
}
.wrap { max-width:1240px; margin:0 auto; padding:0 24px 96px; }

/* ---- masthead ---- */
header.top { padding:56px 0 8px; }
.eyebrow {
  font:500 12px/1 "IBM Plex Mono", ui-monospace, monospace;
  letter-spacing:.18em; text-transform:uppercase; color:var(--faint); margin:0 0 14px;
}
h1 {
  font:700 clamp(34px,5vw,52px)/1.02 Archivo, ui-sans-serif, sans-serif;
  letter-spacing:-.028em; margin:0 0 14px; text-wrap:balance;
}
.lede { font-size:18px; color:var(--muted); max-width:62ch; margin:0 0 8px; }
.lede b { color:var(--ink); font-weight:500; }

/* ---- picks rail ---- */
.rail {
  position:sticky; top:0; z-index:20; margin:28px 0 0;
  background:var(--ground);
  background:color-mix(in srgb, var(--ground) 88%, transparent);
  backdrop-filter:blur(10px);
  border-top:1px solid var(--line); border-bottom:1px solid var(--line);
}
.railin { display:flex; flex-wrap:wrap; align-items:center; gap:10px 20px; padding:12px 0; }
.picks { display:flex; gap:18px; flex-wrap:wrap; }
.pick { display:flex; flex-direction:column; gap:1px; }
.pick dt {
  font:500 10px/1 "IBM Plex Mono", monospace; letter-spacing:.16em;
  text-transform:uppercase; color:var(--faint);
}
.pick dd {
  margin:0; font:500 14px/1.2 "IBM Plex Mono", monospace; color:var(--ink);
  font-variant-numeric:tabular-nums;
}
.pick dd.empty { color:var(--faint); font-weight:400; }
.cmd { margin-left:auto; display:flex; align-items:center; gap:8px; }
.cmd code {
  font:400 13px/1.4 "IBM Plex Mono", monospace; color:var(--ink);
  background:var(--sunk); border:1px solid var(--line-soft);
  border-radius:5px; padding:5px 9px; white-space:nowrap;
}
button.copy, button.wtoggle {
  font:500 12px/1 "IBM Plex Mono", monospace; letter-spacing:.06em; text-transform:uppercase;
  color:var(--muted); background:transparent; border:1px solid var(--line);
  border-radius:5px; padding:7px 11px; cursor:pointer;
}
button.copy:hover, button.wtoggle:hover { color:var(--ink); border-color:var(--muted); }
button.copy:focus-visible, button.wtoggle:focus-visible,
button.mark:focus-visible { outline:2px solid var(--mark); outline-offset:2px; }

/* ---- bands ---- */
section.band { margin-top:64px; }
.bandhead { display:flex; align-items:baseline; gap:14px; flex-wrap:wrap;
            padding-bottom:12px; border-bottom:1px solid var(--line); }
.bandhead h2 {
  font:600 22px/1.1 Archivo, sans-serif; letter-spacing:-.015em; margin:0;
}
.bandhead .n {
  font:500 11px/1 "IBM Plex Mono", monospace; letter-spacing:.14em;
  text-transform:uppercase; color:var(--faint);
}
.bandhead p { margin:0; color:var(--muted); font-size:15px; flex:1 1 34ch; }

/* ---- frames ---- */
.frame {
  display:grid; grid-template-columns:250px minmax(0,1fr); gap:26px;
  padding:26px 0; border-bottom:1px solid var(--line-soft);
}
.frame:last-child { border-bottom:0; }
.capline { display:flex; align-items:baseline; gap:9px; flex-wrap:wrap; }
.caption h3 {
  margin:0; font:600 19px/1.15 Archivo, sans-serif; letter-spacing:-.01em;
}
.caption p { margin:8px 0 0; font-size:14px; color:var(--muted); }
code.flag {
  font:400 12px/1 "IBM Plex Mono", monospace; color:var(--faint);
  background:var(--sunk); border-radius:4px; padding:3px 6px;
}
dl.stats { display:flex; flex-wrap:wrap; gap:4px 18px; margin:14px 0 0; }
dl.stats div { display:flex; flex-direction:column; gap:1px; }
dl.stats dt {
  font:400 10px/1.3 "IBM Plex Mono", monospace; letter-spacing:.12em;
  text-transform:uppercase; color:var(--faint);
}
dl.stats dd {
  margin:0; font:500 13px/1.3 "IBM Plex Mono", monospace;
  font-variant-numeric:tabular-nums; display:flex; align-items:center; gap:6px;
}
.swatch { width:11px; height:11px; border-radius:2px; border:1px solid var(--line); display:inline-block; }

button.mark {
  margin-top:18px; display:inline-flex; align-items:center; gap:9px;
  font:500 12px/1 "IBM Plex Mono", monospace; letter-spacing:.1em; text-transform:uppercase;
  color:var(--muted); background:transparent; border:0; padding:0; cursor:pointer;
}
button.mark .ring {
  width:17px; height:17px; border-radius:50%;
  border:1.5px solid var(--line); flex:none; transition:none;
}
button.mark:hover { color:var(--ink); }
button.mark:hover .ring { border-color:var(--muted); }
@media (prefers-reduced-motion: no-preference) {
  button.mark .ring { transition:border-color .12s ease, background .12s ease; }
}

/* the mark itself — a chinagraph ring, the only hue on the page chrome */
.frame[data-marked="1"] .plate { box-shadow:0 0 0 2px var(--mark); border-radius:8px; }
.frame[data-marked="1"] button.mark { color:var(--mark); }
.frame[data-marked="1"] button.mark .ring {
  border-color:var(--mark); border-width:5px; background:transparent;
}
.frame[data-marked="1"] .caption h3 { color:var(--mark); }

/* ---- the specimens themselves ---- */
.plate { min-width:0; }
.render {
  border-radius:8px; overflow-x:auto; overflow-y:hidden;
  box-shadow:var(--shadow); background:var(--panel);
}
.render pre.pyxray {
  margin:0; padding:18px 20px; white-space:pre;
  font:400 12.5px/1.35 "JetBrains Mono", "DejaVu Sans Mono", "Fira Code", ui-monospace, monospace;
  font-variant-ligatures:none;
}
.note { margin:14px 0 0; font-size:13px; color:var(--faint); }
footer.foot { margin-top:72px; padding-top:22px; border-top:1px solid var(--line); color:var(--muted); font-size:14px; }
footer.foot code { font:400 13px/1 "IBM Plex Mono", monospace; background:var(--sunk); padding:2px 5px; border-radius:4px; }
.saveline { font:400 12px/1 "IBM Plex Mono", monospace; color:var(--faint); }

@media (max-width: 860px) {
  .frame { grid-template-columns:minmax(0,1fr); gap:16px; }
  .rail { position:static; }
  .cmd { margin-left:0; width:100%; }
  .cmd code { white-space:normal; }
}
</style>

<div class="wrap">
<header class="top">
  <p class="eyebrow">pyxray lookdev &middot; 7 themes &middot; 4 layouts &middot; 3 labellings</p>
  <h1>Pick how pyxray should look</h1>
  <p class="lede">Every specimen below is a real render of <b>examples/sketchy.py</b> &mdash; 22 lines that read a token from the environment, delete a directory, download and unpickle a payload, and shell out four times. <b>pyxray</b> scores it 100 and says so in one line. The question is only which way of saying it you want to look at every day.</p>
  <p class="lede">Mark one in each band. Your picks are saved, and the assembled command is at the top.</p>
</header>

<div class="rail"><div class="railin">
  <dl class="picks">
    <div class="pick"><dt>theme</dt><dd id="p-theme" class="empty">unpicked</dd></div>
    <div class="pick"><dt>layout</dt><dd id="p-layout" class="empty">unpicked</dd></div>
    <div class="pick"><dt>labels</dt><dd id="p-icons" class="empty">unpicked</dd></div>
  </dl>
  <div class="cmd">
    <code id="cmdline">pyx script.py</code>
    <button class="copy" type="button" id="copy">Copy</button>
    <button class="wtoggle" type="button" id="wtoggle" aria-pressed="false">Narrow</button>
  </div>
</div></div>

<section class="band">
  <div class="bandhead">
    <h2>Themes</h2><span class="n">7 &middot; card layout &middot; 92 cols</span>
    <p>Layout and labelling held constant. Contrast figures are measured against each theme&rsquo;s own ground; every role in every theme clears 3:1.</p>
  </div>
  __THEMES__
  <p class="note">ANSI and mono resolve their colours from your terminal, so the grounds shown here are stand-ins. They are the ones that survive a strange <code>TERM</code>, a shared tmux session, or a log file.</p>
</section>

<section class="band">
  <div class="bandhead">
    <h2>Layouts</h2><span class="n">4 &middot; blueprint &middot; examples/analysis.py</span>
    <p>A calmer snippet here, because layout is about structure: 60 lines that read CSVs, aggregate with pandas and write two files.</p>
  </div>
  __LAYOUTS__
</section>

<section class="band">
  <div class="bandhead">
    <h2>Effect labelling</h2><span class="n">3 &middot; blueprint &middot; card</span>
    <p>How each capability is named in the timeline and the chip row. This is the axis that decides how much horizontal room the tool needs.</p>
  </div>
  __ICONS__
</section>

<footer class="foot">
  <p>Regenerate any of these against your own code with <code>pyx --contact-sheet sheet.html your_script.py</code>, or flip through them live with <code>pyx --tui your_script.py</code> &mdash; <code>t</code> cycles themes, <code>l</code> layouts, <code>i</code> labelling.</p>
  <p class="saveline" id="saveline">&nbsp;</p>
</footer>
</div>

<script>
(function () {
  "use strict";
  var KINDS = ["theme", "layout", "icons"];
  var state = { theme: null, layout: null, icons: null };
  var db = null;
  var saving = false;

  var saveline = document.getElementById("saveline");
  function say(text) { saveline.textContent = text; }

  function cmd() {
    var parts = ["pyx"];
    if (state.theme) parts.push("-t " + state.theme);
    if (state.layout) parts.push("-l " + state.layout);
    if (state.icons) parts.push("-i " + state.icons);
    parts.push("script.py");
    return parts.join(" ");
  }

  function paint() {
    KINDS.forEach(function (kind) {
      var cell = document.getElementById("p-" + kind);
      cell.textContent = state[kind] || "unpicked";
      cell.classList.toggle("empty", !state[kind]);
    });
    document.querySelectorAll(".frame").forEach(function (frame) {
      var on = state[frame.dataset.kind] === frame.dataset.value;
      frame.dataset.marked = on ? "1" : "0";
      var button = frame.querySelector("button.mark");
      button.setAttribute("aria-pressed", on ? "true" : "false");
      button.querySelector(".marktext").textContent = on ? "Marked" : "Mark";
    });
    document.getElementById("cmdline").textContent = cmd();
  }

  function local(write) {
    try {
      if (write) { window.localStorage.setItem("pyxray.picks", JSON.stringify(state)); return; }
      var raw = window.localStorage.getItem("pyxray.picks");
      if (raw) Object.assign(state, JSON.parse(raw));
    } catch (e) { /* private window, blocked storage — picks just don't persist */ }
  }

  function persist() {
    local(true);
    if (!db) { say("Saved in this browser only."); return; }
    saving = true;
    db.doc("picks/current").set({
      theme: state.theme, layout: state.layout, icons: state.icons,
      at: new Date().toISOString()
    }).then(function () {
      say("Saved " + new Date().toLocaleTimeString());
    }).catch(function (err) {
      say("Could not save (" + err.code + ") — kept in this browser.");
    }).then(function () { saving = false; });
  }

  document.querySelectorAll(".frame button.mark").forEach(function (button) {
    button.addEventListener("click", function () {
      var frame = button.closest(".frame");
      var kind = frame.dataset.kind;
      state[kind] = state[kind] === frame.dataset.value ? null : frame.dataset.value;
      paint();
      persist();
    });
  });

  document.getElementById("copy").addEventListener("click", function (event) {
    var button = event.currentTarget;
    var done = function (text) { button.textContent = text; setTimeout(function () { button.textContent = "Copy"; }, 1400); };
    if (navigator.clipboard) {
      navigator.clipboard.writeText(cmd()).then(function () { done("Copied"); }, function () { done("Select it"); });
    } else { done("Select it"); }
  });

  var wide = true;
  document.getElementById("wtoggle").addEventListener("click", function (event) {
    wide = !wide;
    document.querySelectorAll('.render[data-w]').forEach(function (el) {
      el.hidden = (el.dataset.w === "wide") !== wide;
    });
    event.currentTarget.textContent = wide ? "Narrow" : "Wide";
    event.currentTarget.setAttribute("aria-pressed", wide ? "false" : "true");
  });

  local(false);
  paint();

  if (window.claude && window.claude.use) {
    window.claude.use("db").then(function (store) {
      if (!store) { say("Saved in this browser only."); return; }
      db = store;
      db.doc("picks/current").onSnapshot(function (snap) {
        if (saving || !snap.exists) return;
        var body = snap.data() || {};
        KINDS.forEach(function (kind) { state[kind] = body[kind] || null; });
        paint();
        say("Last saved " + (body.at ? new Date(body.at).toLocaleString() : "—"));
      }, function (err) { say("Live updates stopped (" + err.code + ")."); });
    });
  } else {
    say("Saved in this browser only.");
  }
})();
</script>
"""

page = (PAGE.replace("__THEMES__", band("theme", theme_rows))
            .replace("__LAYOUTS__", band("layout", layout_rows))
            .replace("__ICONS__", band("icons", icon_rows)))
out = S / "contact-sheet.html"
out.write_text(page)
print("wrote", out, out.stat().st_size, "bytes")
