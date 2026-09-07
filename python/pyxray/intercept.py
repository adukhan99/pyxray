"""Sit in front of a Python snippet and record what it will do.

Built for the case that actually happens: an agent firing snippets several
times a second. Two consequences shape everything here.

First, **the render is not for the model.** In most harnesses whatever a hook
or a wrapper writes to stdout or stderr is captured and fed back as tool
output, so drawing a report per snippet would cost tokens, confuse the model,
and scroll past the human anyway. So the default is to append a one-line event
to the feed log and draw nothing. Point ``pyx watch`` at that log in another
pane and you have a live column instead of a wall of text.

Second, **nothing is blocked unless you ask.** ``PYXRAY_GATE`` is unset by
default and pyxray only describes. Note that ``PYXRAY_GATE=0`` is not "no
gate" — it refuses anything with any effect at all, which is almost never what
someone means. ``off`` is spelled out for that reason.

Entry points
------------
``python -m pyxray.intercept -- python3 script.py``
    Render, record, then run. Alias or symlink this as ``python3``.

``python -m pyxray.intercept --hook``
    A ``PreToolUse``-shaped hook. Reads a JSON payload on stdin, finds the
    Python in it, records it, and optionally asks the harness to stop.

Environment
    PYXRAY_OFF=1        do nothing at all
    PYXRAY_LOG          feed log path (default: $XDG_RUNTIME_DIR/pyxray/…)
    PYXRAY_DRAW         never | tty | always   (default: tty)
    PYXRAY_TTY          draw to this device instead, e.g. /dev/pts/7
    PYXRAY_LAYOUT       line | auto | card | dashboard | stack | flow
    PYXRAY_THEME        blueprint | neon | paper | carbon | amber | ansi | mono
    PYXRAY_ICONS        glyph | tag | both
    PYXRAY_WIDTH        columns
    PYXRAY_GATE         off (default) or a risk score to refuse above
    PYXRAY_CONFIRM=1    ask before running
    PYXRAY_MIN_LINES    ignore snippets shorter than this
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from typing import Any

from ._native import engine
from .heredoc import extract_python

__all__ = ["main", "look", "should_run", "gate", "draw_target"]

#: Risk scores at or above this get the full card under ``layout=auto``.
CARD_FROM = 30


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
    return min(shutil.get_terminal_size(fallback=(100, 24)).columns, 180)


def gate() -> int | None:
    """The configured risk ceiling, or None when pyxray is only describing.

    ``off``, ``none`` and an empty value all mean no gate. A bare number is a
    ceiling — including ``0``, which really does refuse everything with an
    effect; that is a legitimate thing to want, just rarely the thing someone
    means when they type it.
    """
    raw = (os.environ.get("PYXRAY_GATE") or "").strip().lower()
    if raw in ("", "off", "none", "-1"):
        return None
    try:
        return max(0, min(100, int(raw)))
    except ValueError:
        print(f"pyxray: ignoring PYXRAY_GATE={raw!r} (want a number, or 'off')",
              file=sys.stderr)
        return None


def draw_target() -> Any:
    """Where the render should go, or None when nobody is watching.

    Defaults to "only if stderr is a real terminal", which is what keeps the
    picture out of a harness's captured tool output.
    """
    mode = (os.environ.get("PYXRAY_DRAW") or "tty").strip().lower()
    if mode == "never":
        return None
    if device := os.environ.get("PYXRAY_TTY"):
        try:
            return open(device, "w", encoding="utf-8")  # noqa: SIM115 — caller closes
        except OSError:
            return None
    if mode == "always":
        return sys.stderr
    try:
        return sys.stderr if sys.stderr.isatty() else None
    except (AttributeError, ValueError):
        return None


def look(code: str, name: str = "<stdin>", source: str = "shim") -> dict[str, Any]:
    """Record a snippet, and draw it if there is somewhere to draw.

    Returns the feed event: ``risk``, ``band``, ``synopsis``, ``notes`` and the
    capability ``mask``. Cheap enough to call on every snippet — the analysis
    is a couple of milliseconds and the render is skipped when unwatched.
    """
    if os.environ.get("PYXRAY_OFF") == "1":
        return {"risk": 0, "band": "inert", "synopsis": "", "notes": [], "blocked": False}

    minimum = _env_int("PYXRAY_MIN_LINES", 0) or 0
    if minimum and len(code.splitlines()) < minimum:
        return {"risk": 0, "band": "inert", "synopsis": "", "notes": [], "blocked": False}

    stream = draw_target()
    kwargs = dict(
        name=name,
        source=source,
        path=os.environ.get("PYXRAY_LOG") or None,
        draw=stream is not None,
        theme=os.environ.get("PYXRAY_THEME", "blueprint"),
        layout=os.environ.get("PYXRAY_LAYOUT", "auto"),
        icons=os.environ.get("PYXRAY_ICONS", "both"),
        width=_width(),
        format="ansi",
    )
    event, rendered = engine().look(code, **kwargs)
    if stream is not None and rendered:
        try:
            stream.write(rendered)
            stream.flush()
        except OSError:
            pass
        finally:
            if stream not in (sys.stderr, sys.stdout):
                stream.close()
    return event


def should_run(event: dict[str, Any]) -> bool:
    """Apply the gate, then the confirmation prompt. Both are opt-in."""
    risk = int(event.get("risk", 0))
    ceiling = gate()
    if ceiling is not None and risk > ceiling:
        worst = event.get("synopsis") or "no summary"
        print(f"pyxray: refusing to run — risk {risk} is over the gate of "
              f"{ceiling} ({worst})", file=sys.stderr)
        return False
    if os.environ.get("PYXRAY_CONFIRM") == "1":
        if not sys.stdin.isatty():
            print("pyxray: PYXRAY_CONFIRM needs a terminal; not running", file=sys.stderr)
            return False
        try:
            answer = input(f"pyxray: risk {risk}. Run it? [y/N] ")
        except (EOFError, KeyboardInterrupt):
            print(file=sys.stderr)
            return False
        if answer.strip().lower() not in ("y", "yes"):
            return False
    return True


def _source_for(argv: list[str]) -> tuple[str, str]:
    """What a `python …` command line is actually going to run."""
    args = argv[1:]
    for i, arg in enumerate(args):
        if arg == "-c" and i + 1 < len(args):
            return args[i + 1], "python -c"
        if arg == "-":
            return sys.stdin.read(), "<stdin>"
        if arg in ("-m", "-W", "-X") and i + 1 < len(args):
            continue
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
    event = look(source, name, source="shim")
    if not should_run(event):
        return 3
    # `-` and a bare interpreter both mean "the script is on stdin", which we
    # have already consumed, so hand it back rather than re-reading it.
    if "-" in argv[1:] or name == "<stdin>":
        proc = subprocess.Popen(argv, stdin=subprocess.PIPE, text=True)
        proc.communicate(source)
        return proc.returncode
    return subprocess.call(argv)


#: Tool names, across harnesses, whose arguments carry Python directly.
CODE_TOOLS = {"execute_code", "python", "run_python", "ipython", "code_interpreter"}
#: Tool names whose arguments carry a shell command that might contain Python.
SHELL_TOOLS = {"bash", "terminal", "shell", "run_command", "execute_command", "sh"}
#: Argument keys to look in, in order of preference.
CODE_KEYS = ("code", "source", "script", "python", "content")
SHELL_KEYS = ("command", "cmd", "commandLine", "args")


def python_in_payload(payload: dict[str, Any]) -> tuple[str, str] | None:
    """Find the Python inside a tool-call payload, whatever shape it arrived in.

    Handles both kinds of tool a harness might offer: one that takes Python
    directly, and one that takes a shell command with Python wrapped inside it.
    """
    tool = (payload.get("tool_name") or payload.get("tool") or "").strip()
    args = payload.get("tool_input") or payload.get("args") or payload.get("input") or {}
    if not isinstance(args, dict):
        return None

    if tool in CODE_TOOLS or not tool:
        for key in CODE_KEYS:
            value = args.get(key)
            if isinstance(value, str) and value.strip():
                return value, f"{tool or 'code'}"

    if tool in SHELL_TOOLS or not tool:
        for key in SHELL_KEYS:
            value = args.get(key)
            if isinstance(value, list):
                value = " ".join(str(v) for v in value)
            if isinstance(value, str) and value.strip():
                code, label = extract_python(value)
                if label != "<stdin>" and code.strip():
                    return code, label
    return None


def _run_hook() -> int:
    """A PreToolUse-shaped hook. Records; asks the harness to stop only if gated.

    The wire format is Claude Code's, which Hermes also accepts, so one script
    serves both. Anything it does not recognise passes through untouched.
    """
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        return 0
    found = python_in_payload(payload)
    if not found:
        return 0
    code, label = found

    session = str(payload.get("session_id") or "")[:8]
    source = payload.get("hook_source") or os.environ.get("PYXRAY_SOURCE") or "hook"
    event = look(code, label, source=f"{source}{':' + session if session else ''}")

    ceiling = gate()
    risk = int(event.get("risk", 0))
    if ceiling is not None and risk > ceiling:
        reason = (f"pyxray: risk {risk} is over the gate of {ceiling} — "
                  f"{event.get('synopsis', '')}")
        json.dump({
            "decision": "block",
            "reason": reason,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "ask",
                "permissionDecisionReason": reason,
            },
        }, sys.stdout)
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
