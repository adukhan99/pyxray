"""Contrast arithmetic, shared by the audit and the lookdev page.

Reads the palettes from ``pyx --list --format json`` rather than by regex over
``theme.rs``: the binary is the one thing that cannot drift from the themes.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

#: Palette roles that must clear 3:1 against the ground. `rule` is a hairline
#: and is allowed to be quiet; `faint` is allowed to be faint.
ROLES = ("fg", "dim", "faint", "accent", "accent_alt", "ok", "warn", "danger")
STRICT = tuple(r for r in ROLES if r not in ("faint",))


def find_pyx() -> str:
    if exe := os.environ.get("PYXRAY_BIN"):
        return exe
    for profile in ("release", "debug"):
        candidate = ROOT / "target" / profile / ("pyx.exe" if os.name == "nt" else "pyx")
        if candidate.is_file():
            return str(candidate)
    if found := shutil.which("pyx"):
        return found
    raise SystemExit("tools/wcag: no pyx binary; run `make release` or set PYXRAY_BIN")


def themes(pyx: str | None = None) -> list[dict]:
    out = subprocess.run([pyx or find_pyx(), "--list", "--format", "json"],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out)["themes"]


def lum(c: tuple[int, int, int]) -> float:
    def f(v: float) -> float:
        v /= 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4

    return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])


def ratio(a: tuple[int, int, int], b: tuple[int, int, int]) -> float:
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def rgb(hex_colour: str) -> tuple[int, int, int]:
    h = hex_colour.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


def audit(theme: dict) -> dict:
    """Per-role contrast against the theme's own ground.

    Returns ``{"roles": {name: ratio}, "effects": {id: ratio}, "body": r,
    "worst": r}``; ``worst`` ignores `rule` and `faint`. 16-colour and
    no-colour themes have no fixed palette and come back with ``None``.
    """
    if not theme.get("truecolor"):
        return {"roles": {}, "effects": {}, "body": None, "worst": None}
    pal = theme["palette"]
    bg = rgb(pal["bg"])
    roles = {k: ratio(rgb(pal[k]), bg) for k in ROLES}
    effects = {k: ratio(rgb(v), bg) for k, v in pal["effects"].items()}
    worst = min(list(roles[k] for k in STRICT) + list(effects.values()))
    return {"roles": roles, "effects": effects, "body": roles["fg"], "worst": worst}
