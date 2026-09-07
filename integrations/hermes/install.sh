#!/bin/sh
# Wire pyxray into Hermes.
#
#   integrations/hermes/install.sh            print the block to paste
#   integrations/hermes/install.sh --append   append it, if it is safe to
#
# Deliberately conservative. ~/.hermes/config.yaml holds the user's model
# settings and can sit next to credentials; rewriting it through a YAML
# round-trip would drop comments and formatting, and a mangled config is a
# much worse outcome than a manual paste. So --append only ever appends, and
# only when there is no `hooks:` key to collide with.
set -e

here=$(cd -- "$(dirname -- "$0")" && pwd)
hook="$(cd -- "$here/.." && pwd)/pyxray-hook"
config="${HERMES_HOME:-$HOME/.hermes}/config.yaml"

block=$(sed "s|PYXRAY_HOOK_PATH|$hook|" "$here/hooks.yaml" | grep -v '^#')

if [ "$1" != "--append" ]; then
    echo "# add to $config:"
    echo "$block"
    echo
    echo "# then, once:"
    echo "#   export HERMES_ACCEPT_HOOKS=1   (or run hermes with --accept-hooks)"
    echo "# Hermes asks for consent per (event, command) on first use and"
    echo "# records it in ~/.hermes/shell-hooks-allowlist.json."
    exit 0
fi

if [ ! -f "$config" ]; then
    echo "pyxray: no $config — run hermes once first" >&2
    exit 1
fi

if grep -q '^hooks:' "$config"; then
    echo "pyxray: $config already has a top-level 'hooks:' key." >&2
    echo "pyxray: merge this in by hand rather than let a script guess:" >&2
    echo "$block" >&2
    exit 1
fi

backup="$config.bak.$(date +%Y%m%d_%H%M%S)"
cp "$config" "$backup"
{ echo; echo "$block"; } >> "$config"
echo "pyxray: appended to $config (backup at $backup)"
echo "pyxray: set HERMES_ACCEPT_HOOKS=1 to consent without a prompt"
echo "pyxray: now run  pyx watch"
