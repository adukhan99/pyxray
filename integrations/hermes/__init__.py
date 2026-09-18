"""pyxray for Hermes — see what the Python is going to do before it runs.

Registers two hooks on ``terminal`` and ``execute_code`` (the two Hermes
surfaces that run Python), a ``/pyxray`` slash command and a ``hermes pyxray``
CLI subcommand.
The logic lives in :mod:`guard`; this file only knows about ``ctx``.

Never breaks a tool call: every hook is wrapped so that a missing engine, a
bad payload or a bug in here logs once and lets the call through untouched.
"""

from __future__ import annotations

import logging
import os
from typing import Any, Optional

from . import guard

logger = logging.getLogger(__name__)

_recent = guard.Recent()
_ctx: Any = None
_warned_missing = False


def _settings() -> guard.Settings:
    get = _ctx.get_config if _ctx is not None and hasattr(_ctx, "get_config") else None
    try:
        return guard.Settings.load(get)
    except Exception:  # noqa: BLE001
        return guard.Settings()


def _note_engine_error(error: str | None) -> None:
    global _warned_missing
    if error and not _warned_missing:
        _warned_missing = True
        logger.warning("pyxray: %s (the plugin stays loaded but describes nothing)", error)


# --------------------------------------------------------------------------- hooks


def _on_pre_tool_call(tool_name: str = "", args: Any = None, session_id: str = "",
                      tool_call_id: str = "", **_: Any) -> Optional[dict[str, Any]]:
    try:
        settings = _settings()
        findings = guard.scan(tool_name, args, settings, session=session_id or "")
        _note_engine_error(findings.error)
        if findings.empty:
            return None
        _recent.remember(tool_name, args, findings, tool_call_id)
        try:
            guard.write_card(findings, settings)
        except Exception as exc:  # noqa: BLE001
            logger.debug("pyxray: card not written: %s", exc)
        return guard.decide(findings, settings)
    except Exception as exc:  # noqa: BLE001 — the observer must not break the observed
        logger.warning("pyxray: pre_tool_call failed: %s", exc)
        return None


def _on_transform_tool_result(tool_name: str = "", args: Any = None, result: Any = None,
                              tool_call_id: str = "", **_: Any) -> Optional[str]:
    try:
        if not isinstance(result, str):
            return None
        findings = _recent.take(tool_name, args, tool_call_id)
        if findings is None or findings.empty:
            return None
        settings = _settings()
        if not guard.wants_warning(findings, settings):
            return None
        block = guard.format_block(findings, settings)
        return result + "\n" + block if block else None
    except Exception as exc:  # noqa: BLE001
        logger.warning("pyxray: transform_tool_result failed: %s", exc)
        return None


# --------------------------------------------------------------------------- slash commands


def _cmd_pyxray(raw_args: str = "", **_: Any) -> str:
    words = (raw_args or "").split()
    sub = words[0].lower() if words else "status"
    rest = words[1:]
    try:
        if sub in ("status", "help", ""):
            return guard.status_text(_settings(), _recent)
        if sub == "last":
            return _cmd_last(rest)
        if sub == "show":
            return _cmd_show(rest)
        if sub == "gate":
            return _cmd_gate(rest)
        if sub == "feed":
            return _cmd_feed(rest)
        return f"pyxray: unknown subcommand {sub!r} — try status, last, show, gate, feed"
    except Exception as exc:  # noqa: BLE001
        return f"pyxray: {exc}"


def _count(rest: list[str], default: int, cap: int = 50) -> int:
    try:
        return max(1, min(cap, int(rest[0]))) if rest else default
    except ValueError:
        return default


def _cmd_last(rest: list[str]) -> str:
    n = _count(rest, 10)
    rows = _recent.last(n)
    if not rows:
        rows_disk = guard.read_recent(n)
        if not rows_disk:
            return "pyxray: nothing seen yet in this session"
        return "\n".join(
            guard.one_line(r.get("event", {}), when=r.get("ts"), label=str(r.get("tool", "")))
            for r in rows_disk
        )
    return "\n".join(
        guard.one_line(f.worst.event, when=f.worst.event.get("ts"), label=f.tool)  # type: ignore[union-attr]
        for f in rows if f.worst is not None
    )


def _cmd_show(rest: list[str]) -> str:
    n = _count(rest, 1)
    rows = _recent.last(n)
    if len(rows) < n:
        return f"pyxray: only {len(rows)} call(s) seen this session"
    worst = rows[-1].worst
    if worst is None:
        return "pyxray: that call had no Python in it"
    mod = guard.pyxray_module()
    theme = "mono" if os.environ.get("NO_COLOR") or os.name == "nt" else os.environ.get("PYXRAY_THEME", "blueprint")
    return mod.render(worst.source, name=worst.label, format="text", theme=theme, layout="card", width=96)


def _cmd_gate(rest: list[str]) -> str:
    if not rest:
        s = _settings()
        return "pyxray gate: " + ("off (describe only)" if s.gate is None else f"{s.gate} → {s.mode}")
    value = guard.parse_gate(rest[0])
    try:
        guard.save_gate(value, _ctx)
    except Exception as exc:  # noqa: BLE001 — managed install, no config module, read-only file
        return f"pyxray: could not save the gate ({exc}) — set PYXRAY_GATE in the environment instead"
    if os.environ.get("PYXRAY_GATE") is not None:
        return (f"pyxray: saved gate={value if value is not None else 'off'} to config, but PYXRAY_GATE="
                f"{os.environ['PYXRAY_GATE']!r} in the environment still wins")
    return "pyxray gate: " + ("off (describe only)" if value is None else f"{value} → {_settings().mode}")


def _cmd_feed(rest: list[str]) -> str:
    n = _count(rest, 15)
    mod = guard.pyxray_module()
    rows = mod.feed()[-n:][::-1]
    if not rows:
        return f"pyxray: feed is empty ({mod.feed_path()})"
    return "\n".join(guard.one_line(r, when=r.get("ts"), label=str(r.get("source", ""))) for r in rows)


# --------------------------------------------------------------------------- CLI


def _cli_setup(parser: Any) -> None:
    sub = parser.add_subparsers(dest="pyxray_cmd")
    sub.add_parser("status", help="engine, feed path and gate")
    feed = sub.add_parser("feed", help="recent feed rows, newest first")
    feed.add_argument("-n", type=int, default=20)
    cards = sub.add_parser("cards", help="where the desktop pane's cards live")
    cards.add_argument("-n", type=int, default=10)


def _cli_handler(args: Any) -> int:
    cmd = getattr(args, "pyxray_cmd", None) or "status"
    if cmd == "status":
        print(guard.status_text(_settings()))
    elif cmd == "feed":
        print(_cmd_feed([str(getattr(args, "n", 20))]))
    elif cmd == "cards":
        root = guard.data_dir()
        print(root)
        for row in guard.read_recent(int(getattr(args, "n", 10))):
            print("  " + guard.one_line(row.get("event", {}), when=row.get("ts"), label=str(row.get("card", ""))))
    return 0


# --------------------------------------------------------------------------- register


def register(ctx: Any) -> None:
    global _ctx
    _ctx = ctx
    # Inside an agent there is no terminal to draw on; the render would land in
    # Hermes' own stderr. The feed and the cards are the picture.
    os.environ.setdefault("PYXRAY_DRAW", "never")

    ctx.register_hook("pre_tool_call", _on_pre_tool_call)
    ctx.register_hook("transform_tool_result", _on_transform_tool_result)
    ctx.register_command(
        "pyxray",
        handler=_cmd_pyxray,
        description="What the Python Hermes ran was going to do: status, last, show, gate, feed.",
        args_hint="[status|last [n]|show [n]|gate <0-100|off>|feed [n]]",
    )
    try:
        ctx.register_cli_command("pyxray", help="pyxray status and feed", setup_fn=_cli_setup, handler_fn=_cli_handler)
    except Exception as exc:  # noqa: BLE001 — older ctx without CLI commands
        logger.debug("pyxray: no CLI command registration: %s", exc)

    try:
        info = guard.engine_status()
        if not info.get("ok"):
            _note_engine_error(str(info.get("error")))
    except Exception:  # noqa: BLE001
        pass
