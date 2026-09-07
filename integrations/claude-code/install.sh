#!/bin/sh
# Wire pyxray into Claude Code's settings without the plugin system.
#   integrations/claude-code/install.sh [--global]
set -e

here=$(cd -- "$(dirname -- "$0")" && pwd)
hook="$(cd -- "$here/.." && pwd)/pyxray-hook"

if [ "$1" = "--global" ]; then
    target="$HOME/.claude/settings.json"
else
    target=".claude/settings.json"
fi
mkdir -p "$(dirname "$target")"

python3 - "$target" "$hook" <<'PY'
import json, sys, pathlib

target, hook = pathlib.Path(sys.argv[1]), sys.argv[2]
settings = {}
if target.exists():
    try:
        settings = json.loads(target.read_text() or "{}")
    except json.JSONDecodeError:
        print(f"pyxray: {target} is not valid JSON; not touching it", file=sys.stderr)
        raise SystemExit(1)

hooks = settings.setdefault("hooks", {}).setdefault("PreToolUse", [])
entry = {"matcher": "Bash",
         "hooks": [{"type": "command", "command": hook, "timeout": 5}]}

for existing in hooks:
    for h in existing.get("hooks", []):
        if "pyxray-hook" in str(h.get("command", "")):
            h["command"] = hook
            break
    else:
        continue
    break
else:
    hooks.append(entry)

target.write_text(json.dumps(settings, indent=2) + "\n")
print(f"pyxray: wired into {target}")
PY

echo "now run:  pyx watch"
