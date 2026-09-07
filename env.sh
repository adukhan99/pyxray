# Optional build environment.  Source it; it does nothing you did not ask for.
#
#   source env.sh
#
# Two reasons you might want it:
#
#   * your $HOME is small or quota'd, and cargo's registry plus a release
#     build runs to a couple of gigabytes;
#   * your toolchain lives somewhere other than the default.
#
# Set PYXRAY_BUILD_ROOT to move cargo's caches and the build output wholesale,
# or set CARGO_HOME / RUSTUP_HOME / CARGO_TARGET_DIR yourself and this leaves
# them alone.  Either way it puts ./python on PYTHONPATH so the package is
# importable from a checkout.
#
# env.local.sh, if it exists, is sourced first and is gitignored — machine
# specifics belong there, not in this file.

_pyxray_repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-$0}")" && pwd)

if [ -f "$_pyxray_repo/env.local.sh" ]; then
    . "$_pyxray_repo/env.local.sh"
fi

if [ -n "${PYXRAY_BUILD_ROOT:-}" ]; then
    : "${CARGO_HOME:=$PYXRAY_BUILD_ROOT/cargo}"
    : "${RUSTUP_HOME:=$PYXRAY_BUILD_ROOT/rustup}"
    : "${CARGO_TARGET_DIR:=$PYXRAY_BUILD_ROOT/target}"
    export CARGO_HOME RUSTUP_HOME CARGO_TARGET_DIR
fi

if [ -n "${CARGO_HOME:-}" ] && [ -d "$CARGO_HOME/bin" ]; then
    PATH="$CARGO_HOME/bin:$PATH"
    export PATH
fi

PYTHONPATH="$_pyxray_repo/python${PYTHONPATH:+:$PYTHONPATH}"
export PYTHONPATH

unset _pyxray_repo
