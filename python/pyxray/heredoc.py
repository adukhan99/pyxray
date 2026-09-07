"""Pull the Python out of the shell command a model typically emits.

This mirrors the Rust implementation so the pure-Python fallback path behaves
the same way. Anything that is not recognisably a wrapped snippet is returned
untouched, on the assumption that it was already Python.
"""

from __future__ import annotations

import re

_PREFIXES = ("python3 ", "python ", "uv run python", "py ")
_HEREDOC = re.compile(r"<<-?\s*(?:'([^']+)'|\"([^\"]+)\"|([A-Za-z_][A-Za-z0-9_]*))")


def extract_python(command: str) -> tuple[str, str]:
    """Return ``(source, label)`` for a command, or the input unchanged."""
    text = command.lstrip()
    if not any(text.startswith(p) for p in _PREFIXES):
        return command, "<stdin>"

    match = _HEREDOC.search(text)
    if match:
        delim = match.group(1) or match.group(2) or match.group(3)
        body = text[match.end():]
        body = body[1:] if body.startswith("\n") else body
        lines = body.split("\n")
        for i, line in enumerate(lines):
            if line.lstrip("\t").rstrip("\r") == delim:
                return "\n".join(lines[:i]) + ("\n" if i else ""), f"heredoc <<{delim}"
        return body, f"heredoc <<{delim} (unterminated)"

    dash_c = re.search(r"-c\s*(['\"])(.*)\1", text, re.S)
    if dash_c:
        return dash_c.group(2), "python -c"
    return command, "<stdin>"
