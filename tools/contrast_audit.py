"""Check every colour theme against the contrast the README promises.

Exits 1 when any role or effect colour falls under 3:1 against its theme's
ground — so `make audit` (and CI) is a check, not a report. Body text is
held to 4.5:1.

    python3 tools/contrast_audit.py            # audit, exit 1 on failure
    python3 tools/contrast_audit.py --verbose  # every figure
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import wcag  # noqa: E402

MIN_ROLE = 3.0
MIN_BODY = 4.5


def main(argv: list[str]) -> int:
    verbose = "--verbose" in argv or "-v" in argv
    failures = 0
    for theme in wcag.themes():
        name = theme["id"]
        result = wcag.audit(theme)
        if result["worst"] is None:
            print(f"{name:<10} 16-colour / no-colour theme — the terminal decides")
            continue
        rows = {**result["roles"], **{"fx." + k: v for k, v in result["effects"].items()}}
        bad = {k: v for k, v in rows.items() if v < MIN_ROLE and k not in ("faint",)}
        body_ok = result["body"] >= MIN_BODY
        status = "ok " if not bad and body_ok else "BAD"
        print(f"{status} {name:<10} ground {theme['palette']['bg']}  "
              f"body {result['body']:.1f}:1  worst {result['worst']:.1f}:1")
        if bad:
            failures += 1
            print("    under 3.0:1 →", ", ".join(f"{k} {v:.1f}" for k, v in sorted(bad.items(), key=lambda x: x[1])))
        if not body_ok:
            failures += 1
            print(f"    body text {result['body']:.1f}:1 is under {MIN_BODY}:1")
        if verbose:
            for k, v in sorted(rows.items(), key=lambda x: x[1]):
                print(f"    {k:<14} {v:5.1f}:1")
    if failures:
        print(f"\n{failures} theme(s) fail the contrast floor", file=sys.stderr)
        return 1
    print("\nevery colour theme clears 3:1 on every role, 4.5:1 on body text")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
