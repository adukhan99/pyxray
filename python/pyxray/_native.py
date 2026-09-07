"""Locate the analysis engine.

Prefers the compiled extension that ships beside this file. Falls back to the
``pyx`` binary, so a checkout with a built CLI but no built extension still
works — which is the common case when someone has just cloned and run
``cargo build``.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

__all__ = ["Engine", "engine", "EngineError"]

_ROOT = Path(__file__).resolve().parent.parent.parent


class EngineError(RuntimeError):
    """No usable pyxray engine could be found."""


class Engine:
    """Whichever of the two backends is available, behind one interface."""

    kind: str

    def analyze(self, source: str, name: str = "<stdin>") -> dict[str, Any]:
        raise NotImplementedError

    def render(self, source: str, **kw: Any) -> str:
        raise NotImplementedError

    def extract(self, command: str) -> tuple[str, str]:
        raise NotImplementedError

    def themes(self) -> list[tuple[str, str, str]]:
        raise NotImplementedError

    def layouts(self) -> list[tuple[str, str]]:
        raise NotImplementedError


class _Extension(Engine):
    kind = "extension"

    def __init__(self, mod: Any) -> None:
        self._m = mod

    def analyze(self, source: str, name: str = "<stdin>") -> dict[str, Any]:
        return json.loads(self._m.analyze_json(source, name))

    def render(self, source: str, **kw: Any) -> str:
        return self._m.render(source, **kw)

    def extract(self, command: str) -> tuple[str, str]:
        return tuple(self._m.extract(command))  # type: ignore[return-value]

    def themes(self) -> list[tuple[str, str, str]]:
        return self._m.themes()

    def layouts(self) -> list[tuple[str, str]]:
        return self._m.layouts()


class _Binary(Engine):
    kind = "binary"

    def __init__(self, exe: str) -> None:
        self._exe = exe

    def _run(self, args: list[str], source: str) -> str:
        proc = subprocess.run(
            [self._exe, *args],
            input=source,
            capture_output=True,
            text=True,
        )
        if proc.returncode not in (0,):
            raise EngineError(proc.stderr.strip() or f"pyx exited {proc.returncode}")
        return proc.stdout

    def analyze(self, source: str, name: str = "<stdin>") -> dict[str, Any]:
        return json.loads(self._run(["--format", "json"], source))

    def render(self, source: str, **kw: Any) -> str:
        args = ["--format", str(kw.get("format", "ansi"))]
        for flag, key in (
            ("--theme", "theme"),
            ("--layout", "layout"),
            ("--width", "width"),
            ("--height", "height"),
            ("--icons", "icons"),
            ("--depth", "depth"),
        ):
            value = kw.get(key)
            if value is not None:
                args += [flag, str(value)]
        if kw.get("code"):
            args.append("--code")
        if kw.get("gutter") is False:
            args.append("--no-gutter")
        return self._run(args, source)

    def extract(self, command: str) -> tuple[str, str]:
        # The binary does this internally; replicate just enough here that the
        # fallback path is not crippled.
        from .heredoc import extract_python

        return extract_python(command)

    def themes(self) -> list[tuple[str, str, str]]:
        out = subprocess.run([self._exe, "--list"], capture_output=True, text=True).stdout
        rows: list[tuple[str, str, str]] = []
        section = None
        for line in out.splitlines():
            if line.endswith(":"):
                section = line[:-1]
            elif section == "themes" and line.startswith("  "):
                ident, _, blurb = line.strip().partition(" ")
                rows.append((ident, ident.title(), blurb.strip()))
        return rows

    def layouts(self) -> list[tuple[str, str]]:
        out = subprocess.run([self._exe, "--list"], capture_output=True, text=True).stdout
        rows: list[tuple[str, str]] = []
        section = None
        for line in out.splitlines():
            if line.endswith(":"):
                section = line[:-1]
            elif section == "layouts" and line.startswith("  "):
                ident, _, blurb = line.strip().partition(" ")
                rows.append((ident, blurb.strip()))
        return rows


def _find_binary() -> str | None:
    if override := os.environ.get("PYXRAY_BIN"):
        return override if Path(override).exists() else None
    found = shutil.which("pyx")
    if found:
        return found
    for profile in ("release", "debug"):
        candidate = _ROOT / "target" / profile / "pyx"
        if candidate.exists():
            return str(candidate)
    return None


_cached: Engine | None = None


def engine() -> Engine:
    """The best available backend, resolved once."""
    global _cached
    if _cached is not None:
        return _cached
    try:
        from . import _pyxray  # type: ignore[attr-defined]

        _cached = _Extension(_pyxray)
        return _cached
    except ImportError:
        pass
    exe = _find_binary()
    if exe:
        _cached = _Binary(exe)
        return _cached
    raise EngineError(
        "no pyxray engine found — build one with `cargo build --release` "
        "(then `make install-ext`), or set PYXRAY_BIN to a pyx binary"
    )
