# pyxray shell integration — source this from ~/.bashrc or ~/.zshrc.
#
#   source /path/to/py_vis_interceptor/shell/pyxray.sh
#
# Wraps python3 so every script, heredoc and -c one-liner gets an X-ray on the
# way past. Nothing is blocked unless you ask for it:
#
#   export PYXRAY_GATE=60      refuse to run anything scarier than this (0-100)
#   export PYXRAY_CONFIRM=1    always ask first
#   export PYXRAY_LAYOUT=card  card | dashboard | stack | flow
#   export PYXRAY_THEME=carbon blueprint | neon | paper | carbon | amber | ansi | mono
#   export PYXRAY_MIN_LINES=3  skip the X-ray for one-liners
#   export PYXRAY_OFF=1        turn the whole thing off for this shell

_PYXRAY_ROOT="${_PYXRAY_ROOT:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-${(%):-%x}}")/.." && pwd)}"
export PYTHONPATH="${_PYXRAY_ROOT}/python${PYTHONPATH:+:$PYTHONPATH}"
export PYXRAY_BIN="${PYXRAY_BIN:-${_PYXRAY_ROOT}/target/release/pyx}"
[ -x "$PYXRAY_BIN" ] || export PYXRAY_BIN="${_PYXRAY_ROOT}/target/debug/pyx"

# The real interpreter, resolved once so the wrapper cannot recurse into itself.
_PYXRAY_PYTHON="${_PYXRAY_PYTHON:-$(command -v python3)}"

python3() {
  if [ "${PYXRAY_OFF:-0}" = "1" ] || [ $# -eq 0 ]; then
    command "$_PYXRAY_PYTHON" "$@"
  else
    command "$_PYXRAY_PYTHON" -m pyxray.intercept -- "$_PYXRAY_PYTHON" "$@"
  fi
}

# `pyx` on PATH without installing anything.
pyx() { command "$PYXRAY_BIN" "$@"; }

# Pipe a whole shell command in and see the Python inside it:
#   pyxcmd "python3 <<'EOF' ... EOF"
pyxcmd() { printf '%s' "$*" | command "$PYXRAY_BIN" --stdin-is-command "${PYXRAY_ARGS[@]}"; }
