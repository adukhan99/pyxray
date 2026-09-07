#!/bin/sh
# Put the pyxray shim on PATH for this shell, or print what to add to your rc.
#
#   . integrations/shim/install.sh          # this shell only
#   integrations/shim/install.sh --print    # show the lines to keep
set -e

here=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$here/../.." && pwd)

bin="$root/target/release/pyx"
[ -x "$bin" ] || bin="$root/target/debug/pyx"

if [ "$1" = "--print" ]; then
    cat <<EOF
# pyxray — see what Python does before it runs
export PATH="$here:\$PATH"
export PYTHONPATH="$root/python\${PYTHONPATH:+:\$PYTHONPATH}"
export PYXRAY_BIN="$bin"
# Optional:
#   export PYXRAY_DRAW=never     # feed only; watch it with: pyx watch
#   export PYXRAY_TTY=/dev/pts/7 # or draw to a dedicated pane
#   export PYXRAY_GATE=off       # the default; a number refuses above it
EOF
    exit 0
fi

PATH="$here:$PATH"
PYTHONPATH="$root/python${PYTHONPATH:+:$PYTHONPATH}"
PYXRAY_BIN="$bin"
export PATH PYTHONPATH PYXRAY_BIN

echo "pyxray shim active for this shell."
echo "  feed: $("$bin" --list 2>/dev/null | tail -1 | tr -d ' ')"
echo "  watch it with: $bin watch"
echo "  make it permanent: $here/install.sh --print >> ~/.bashrc"
