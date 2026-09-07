#!/bin/sh
# Open the pyxray feed in a tmux split beside whatever you are already doing.
#
#   integrations/tmux/pyxray-pane.sh              a 30% split below
#   integrations/tmux/pyxray-pane.sh -h -p 40     a 40% split to the right
#   integrations/tmux/pyxray-pane.sh --tty        also point renders at it
#
# --tty prints the export line for PYXRAY_TTY, which makes the shim and the
# hooks draw the full card into this pane instead of only feeding it rows.
set -e

here=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$here/../.." && pwd)
bin="$root/target/release/pyx"
[ -x "$bin" ] || bin="$root/target/debug/pyx"

if [ ! -x "$bin" ]; then
    echo "pyxray: no binary at $bin — run 'make release' first" >&2
    exit 1
fi

if [ -z "$TMUX" ]; then
    echo "pyxray: not inside tmux; just run '$bin watch'" >&2
    exit 1
fi

split="-v"
size="30"
want_tty=""
while [ $# -gt 0 ]; do
    case "$1" in
        -h|--horizontal) split="-h" ;;
        -v|--vertical)   split="-v" ;;
        -p|--percent)    shift; size="$1" ;;
        --tty)           want_tty="1" ;;
        *) echo "pyxray: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done

pane=$(tmux split-window "$split" -p "$size" -P -F '#{pane_id}' "$bin watch")
tmux select-pane -t "$pane" -T "pyxray"
# Hand focus back: you opened this to watch, not to type in it.
tmux last-pane

if [ -n "$want_tty" ]; then
    device=$(tmux display-message -p -t "$pane" '#{pane_tty}')
    echo "export PYXRAY_TTY=$device"
    echo "export PYXRAY_DRAW=tty"
    echo "# eval \"\$($0 --tty)\" to apply, or paste the two lines above"
fi
