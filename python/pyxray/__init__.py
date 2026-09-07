"""pyxray — see what a Python snippet will do before you run it.

    >>> import pyxray
    >>> report = pyxray.analyze("import os; os.remove('x')")
    >>> report["meta"]["synopsis"]
    'DELETES x'
    >>> print(pyxray.render(open("script.py").read(), theme="carbon"))

The heavy lifting is Rust; this package is the ergonomic surface and the
interception glue. See :mod:`pyxray.intercept` for the part that sits in front
of a command a model just handed you.
"""

from __future__ import annotations

from typing import Any

from ._native import Engine, EngineError, engine

__all__ = [
    "analyze",
    "render",
    "extract",
    "themes",
    "layouts",
    "backend",
    "Engine",
    "EngineError",
]

__version__ = "0.1.0"


def analyze(source: str, name: str = "<stdin>") -> dict[str, Any]:
    """Analyse a snippet and return the full report as a dict.

    Never raises on bad syntax: an unparseable snippet still yields a report,
    with the errors under ``diagnostics`` and ``meta.parsed`` set to False.
    """
    return engine().analyze(source, name)


def render(
    source: str,
    *,
    name: str = "<stdin>",
    theme: str = "blueprint",
    layout: str = "card",
    format: str = "ansi",
    width: int = 100,
    height: int | None = None,
    icons: str = "both",
    depth: int = 6,
    gutter: bool = True,
    code: bool = False,
) -> str:
    """Render a snippet to ANSI, plain text, HTML or SVG."""
    return engine().render(
        source,
        name=name,
        theme=theme,
        layout=layout,
        format=format,
        width=width,
        height=height,
        icons=icons,
        depth=depth,
        gutter=gutter,
        code=code,
    )


def extract(command: str) -> tuple[str, str]:
    """Pull the Python out of ``python3 <<'EOF' … EOF`` and friends."""
    return engine().extract(command)


def themes() -> list[tuple[str, str, str]]:
    """Available themes as ``(id, name, blurb)``."""
    return engine().themes()


def layouts() -> list[tuple[str, str]]:
    """Available layouts as ``(id, blurb)``."""
    return engine().layouts()


def backend() -> str:
    """Which engine is in use: ``"extension"`` or ``"binary"``."""
    return engine().kind
