"""Render every theme, layout and labelling mode to HTML fragments.

Feeds tools/build_lookdev.py. Run from the repo root after `make release`.
"""

import json
import pathlib
import re
import subprocess
import sys

PYX = sys.argv[1] if len(sys.argv) > 1 else "./target/release/pyx"
OUT = pathlib.Path("build/lookdev")
THEMES = ["blueprint", "neon", "paper", "carbon", "amber", "ansi", "mono"]


def frag(example, theme, layout, width, icons="both", depth=6):
    return subprocess.run(
        [PYX, "--format", "html", "--fragment", "-w", str(width), "-t", theme,
         "-l", layout, "-i", icons, "--depth", str(depth), f"examples/{example}.py"],
        capture_output=True, text=True, check=True,
    ).stdout


def contrast_table():
    """Per-theme WCAG figures, read straight out of the theme definitions."""
    src = pathlib.Path("crates/pyxray-render/src/theme.rs").read_text()

    def lum(c):
        def f(v):
            v /= 255
            return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
        return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])

    def ratio(a, b):
        la, lb = lum(a), lum(b)
        hi, lo = max(la, lb), min(la, lb)
        return (hi + 0.05) / (lo + 0.05)

    def h2r(h):
        return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))

    out = {}
    for name, body in re.findall(r"pub const (\w+): Theme = Theme \{(.*?)\n\};", src, re.S):
        blurb = re.search(r'blurb: "(.*?)"', body).group(1)
        if "Color::Reset" in body:
            out[name.lower()] = {"blurb": blurb, "body": None, "worst": None}
            continue
        pal = dict(re.findall(r"(\w+): rgb\(0x([0-9a-f]{6})\)", body))
        fx = re.findall(r"rgb\(0x([0-9a-f]{6})\)", body.split("effects: [")[1])
        bg = h2r(pal["bg"])
        roles = [ratio(h2r(pal[k]), bg)
                 for k in ("fg", "dim", "faint", "accent", "accent_alt", "ok", "warn", "danger")]
        roles += [ratio(h2r(h), bg) for h in fx]
        out[name.lower()] = {
            "blurb": blurb,
            "body": round(ratio(h2r(pal["fg"]), bg), 1),
            "worst": round(min(roles), 1),
            "bg": "#" + pal["bg"],
        }
    return out


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    data = {"themeWide": {}, "themeNarrow": {}, "layouts": {}, "icons": {}, "ground": {}}
    for theme in THEMES:
        data["themeWide"][theme] = frag("sketchy", theme, "card", 92)
        data["themeNarrow"][theme] = frag("sketchy", theme, "card", 46)
        match = re.search(r"background:(#[0-9a-f]{6})", data["themeWide"][theme])
        data["ground"][theme] = match.group(1)
    for layout, width in [("card", 96), ("dashboard", 118), ("stack", 96), ("flow", 96)]:
        data["layouts"][layout] = frag("analysis", "blueprint", layout, width, depth=4)
    for icons in ["glyph", "tag", "both"]:
        data["icons"][icons] = frag("sketchy", "blueprint", "card", 88, icons=icons)

    (OUT / "frags.json").write_text(json.dumps(data))
    (OUT / "themeinfo.json").write_text(json.dumps(contrast_table(), indent=1))
    print(f"wrote {OUT}/frags.json and themeinfo.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
