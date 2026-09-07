#!/bin/sh
# Install the pyxray plugin for OpenCode.
#
#   integrations/opencode/install.sh             ~/.config/opencode/plugins
#   integrations/opencode/install.sh --project   .opencode/plugins
set -e

here=$(cd -- "$(dirname -- "$0")" && pwd)
hook="$(cd -- "$here/.." && pwd)/pyxray-hook"

if [ "$1" = "--project" ]; then
    target=".opencode/plugins"
else
    target="${XDG_CONFIG_HOME:-$HOME/.config}/opencode/plugins"
fi
mkdir -p "$target"

sed "s|\"PYXRAY_HOOK_PATH\"|\"$hook\"|" "$here/pyxray.js" > "$target/pyxray.js"
echo "pyxray: installed $target/pyxray.js"
echo "pyxray: hook -> $hook"
echo "pyxray: now run  pyx watch"
