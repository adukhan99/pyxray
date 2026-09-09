#!/bin/sh
# Wire pyxray into Claude Code's settings without the plugin system.
#
#   integrations/claude-code/install.sh            .claude/settings.json (project)
#   integrations/claude-code/install.sh --global   ~/.claude/settings.json
#
# Prefer the plugin when you can: `/plugin marketplace add adukhan99/pyxray`
# then `/plugin install pyxray@pyxray` works in Claude Code and in the Claude
# desktop app alike, and needs no absolute paths.
set -e

here=$(cd -- "$(dirname -- "$0")" && pwd)
hook="$here/hooks/pyxray-hook"

if [ "$1" = "--global" ]; then
    target="$HOME/.claude/settings.json"
else
    target=".claude/settings.json"
fi
mkdir -p "$(dirname "$target")"

py=python3
command -v python3 >/dev/null 2>&1 || py=python

"$py" - "$target" "$hook" <<'PY'
import json, sys, pathlib

target, hook = pathlib.Path(sys.argv[1]), sys.argv[2]
command = f'sh "{hook}"'
settings = {}
if target.exists():
    try:
        settings = json.loads(target.read_text() or "{}")
    except json.JSONDecodeError:
        print(f"pyxray: {target} is not valid JSON; not touching it", file=sys.stderr)
        raise SystemExit(1)

hooks = settings.setdefault("hooks", {}).setdefault("PreToolUse", [])
for matcher in ("Bash", "NotebookEdit"):
    for existing in hooks:
        if existing.get("matcher") != matcher:
            continue
        for h in existing.get("hooks", []):
            if "pyxray-hook" in str(h.get("command", "")):
                h["command"] = command
                h["timeout"] = 5
                break
        else:
            existing.setdefault("hooks", []).append(
                {"type": "command", "command": command, "timeout": 5})
        break
    else:
        hooks.append({"matcher": matcher,
                      "hooks": [{"type": "command", "command": command, "timeout": 5}]})

target.write_text(json.dumps(settings, indent=2) + "\n")
print(f"pyxray: wired into {target}")
PY

echo "now run:  pyx watch"
