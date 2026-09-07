"""Sit in front of a Python snippet and show what it will do.

Two entry points, both driven by the same environment variables:

``python -m pyxray.intercept -- python3 script.py``
    Renders the X-ray, then runs the command. Symlink or alias this as
    ``python3`` and every snippet gets looked at on the way past.

``python -m pyxray.intercept --hook``
    Reads a Claude Code ``PreToolUse`` hook payload on stdin, renders the
    X-ray for any Bash command that turns out to contain Python, and lets the
    command through. Add ``PYXRAY_GATE`` to make it stop asking nicely.

Environment
    PYXRAY_OFF=1        disable entirely, for when you just want the snippet run
    PYXRAY_THEME        theme id (default: blueprint)
    PYXRAY_LAYOUT       layout id (default: card)
    PYXRAY_WIDTH        columns (default: the terminal's, else 100)
    PYXRAY_ICONS        glyph | tag | both
    PYXRAY_GATE         refuse to run above this risk score, 0-100
    PYXRAY_CONFIRM=1    ask before running
    PYXRAY_MIN_LINES    skip the X-ray for snippets shorter than this
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from typing import Any

from . import analyze, render
from .heredoc import extract_python

__all__ = ["main", "look", "should_run"]


def _env_int(name: str, default: int | None = None) -> int | None:
    raw = os.environ.get(name)
    if raw is None or not raw.strip():
        return default
    try:
        return int(raw)
    except ValueError:
        return default


def _width() -> int:
    if explicit := _env_int("PYXRAY_WIDTH"):
        return explicit
    return min(shutil.get_terminal_size(fallback=(100, 24)).columns, 160)


def look(source: str, name: str = "<stdin>", stream: Any = sys.stderr) -> dict[str, Any]:
    """Render the X-ray for a snippet and return its report.

    Draws to stderr by default so it never contaminates a pipeline whose real
    output is the program's.
    """
    report = analyze(source, name)
    minimum = _env_int("PYXRAY_MIN_LINES", 0) or 0
    if report["metrics"]["lines_total"] >= minimum:
        art = render(
            source,
            name=name,
            theme=os.environ.get("PYXRAY_THEME", "blueprint"),
            layout=os.environ.get("PYXRAY_LAYOUT", "card"),
            format="ansi" if stream.isatty() else "text",
            width=_width(),
            icons=os.environ.get("PYXRAY_ICONS", "both"),
        )
        stream.write(art)
        stream.flush()
    return report


def should_run(report: dict[str, Any]) -> bool:
    """Apply the gate and the confirmation prompt, in that order."""
    risk = report["metrics"]["risk"]
    gate = _env_int("PYXRAY_GATE")
    if gate is not None and risk > gate:
        print(
            f"pyxray: refusing to run — risk {risk} is over the gate of {gate}",
            file=sys.stderr,
        )
        return False
    if os.environ.get("PYXRAY_CONFIRM") == "1":
        if not sys.stdin.isatty():
            print("pyxray: --confirm needs a terminal; not running", file=sys.stderr)
            return False
        try:
            answer = input("pyxray: run this? [y/N] ")
        except (EOFError, KeyboardInterrupt):
            print(file=sys.stderr)
            return False
        if answer.strip().lower() not in ("y", "yes"):
            return False
    return True


def _source_for(argv: list[str]) -> tuple[str, str]:
    """Work out what a `python …` command line is actually going to run."""
    args = argv[1:]
    for i, arg in enumerate(args):
        if arg == "-c" and i + 1 < len(args):
            return args[i + 1], "python -c"
        if arg == "-":
            return sys.stdin.read(), "<stdin>"
        if not arg.startswith("-"):
            try:
                with open(arg, encoding="utf-8") as fh:
                    return fh.read(), os.path.basename(arg)
            except OSError:
                return "", arg
    if not sys.stdin.isatty():
        return sys.stdin.read(), "<stdin>"
    return "", "<repl>"


def _run_wrapped(argv: list[str]) -> int:
    source, name = _source_for(argv)
    if not source or os.environ.get("PYXRAY_OFF") == "1":
        return subprocess.call(argv)
    report = look(source, name)
    if not should_run(report):
        return 3
    # `-` and a bare interpreter both mean "the script is on stdin", which we
    # have already consumed, so hand it back rather than re-reading it.
    if "-" in argv[1:] or name == "<stdin>":
        proc = subprocess.Popen(argv, stdin=subprocess.PIPE, text=True)
        proc.communicate(source)
        return proc.returncode
    return subprocess.call(argv)


def _run_hook() -> int:
    """Claude Code PreToolUse hook: X-ray any Bash command carrying Python."""
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return 0
    command = (payload.get("tool_input") or {}).get("command", "")
    if not command:
        return 0
    source, label = extract_python(command)
    if label == "<stdin>" or not source.strip():
        return 0
    report = look(source, label, stream=sys.stderr)
    gate = _env_int("PYXRAY_GATE")
    risk = report["metrics"]["risk"]
    if gate is not None and risk > gate:
        json.dump(
            {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "ask",
                    "permissionDecisionReason": (
                        f"pyxray: risk {risk} is over the gate of {gate} — "
                        f"{report['meta']['synopsis']}"
                    ),
                }
            },
            sys.stdout,
        )
    return 0


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if args and args[0] == "--hook":
        return _run_hook()
    if args and args[0] == "--":
        args = args[1:]
    if not args:
        print(__doc__, file=sys.stderr)
        return 2
    return _run_wrapped(args)


if __name__ == "__main__":
    raise SystemExit(main())
