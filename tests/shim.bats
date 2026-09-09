#!/usr/bin/env bats
# The PATH shim, end to end. Needs: bats, a python3, and either the built
# extension or a pyx binary (target/debug/pyx after `cargo build`).

setup() {
    ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
    SHIM="$ROOT/integrations/shim"
    [ -e "$SHIM/python" ] || ln -s python3 "$SHIM/python"
    export PATH="$SHIM:$PATH"
    export PYTHONPATH="$ROOT/python"
    export PYXRAY_DRAW=never
    export PYXRAY_LOG="$BATS_TEST_TMPDIR/feed.jsonl"
    if [ -z "${PYXRAY_BIN:-}" ] && [ -x "$ROOT/target/debug/pyx" ]; then
        export PYXRAY_BIN="$ROOT/target/debug/pyx"
    fi
    unset PYXRAY_GATE PYXRAY_OFF
}

@test "the shim is what PATH resolves, and it finds the real interpreter" {
    run command -v python3
    [ "$output" = "$SHIM/python3" ]
    run python3 --version
    [ "$status" -eq 0 ]
    [[ "$output" == Python* ]]
}

@test "a -c snippet runs and lands in the feed" {
    run python3 -c 'print("hello from -c")'
    [ "$status" -eq 0 ]
    [[ "$output" == *"hello from -c"* ]]
    [ "$(wc -l < "$PYXRAY_LOG")" -eq 1 ]
}

@test "a piped script is analysed and still runs" {
    run bash -c 'echo "print(\"piped\")" | python3'
    [ "$status" -eq 0 ]
    [[ "$output" == *piped* ]]
    [ "$(wc -l < "$PYXRAY_LOG")" -eq 1 ]
}

@test "python (no 3) is caught too" {
    run python -c 'print("alias")'
    [ "$status" -eq 0 ]
    [[ "$output" == *alias* ]]
    [ "$(wc -l < "$PYXRAY_LOG")" -eq 1 ]
}

@test "interpreter flags with values are read correctly" {
    printf 'print("flagged")\n' > "$BATS_TEST_TMPDIR/s.py"
    run python3 -X faulthandler "$BATS_TEST_TMPDIR/s.py"
    [ "$status" -eq 0 ]
    [[ "$output" == *flagged* ]]
    grep -q '"name":"s.py"' "$PYXRAY_LOG"
}

@test "the gate refuses with exit 3 and says why" {
    export PYXRAY_GATE=10
    run python3 -c 'import shutil; shutil.rmtree("/nope/never")'
    [ "$status" -eq 3 ]
    [[ "$output" == *"refusing to run"* ]]
}

@test "PYXRAY_OFF passes straight through and records nothing" {
    export PYXRAY_OFF=1
    run python3 -c 'print("off")'
    [ "$status" -eq 0 ]
    [ ! -e "$PYXRAY_LOG" ]
}

@test "when pyxray is not importable the command still runs" {
    export PYTHONPATH=/nowhere
    run python3 -c 'import sys; print("no package", sys.version_info[0])'
    [ "$status" -eq 0 ]
    [[ "$output" == *"no package 3"* ]]
}

@test "a shell && chain through the shim keeps its exit codes" {
    run bash -c 'python3 -c "import os" && echo ok'
    [ "$status" -eq 0 ]
    [[ "$output" == *ok* ]]
    run bash -c 'python3 -c "raise SystemExit(7)"; echo "rc=$?"'
    [[ "$output" == *"rc=7"* ]]
}
