# pyxray

**See what a Python snippet will do before you run it.**

Language models hand you Python constantly — `python3 <<'EOF' … EOF`, a `-c`
one-liner, a file to run. Reading forty lines of someone else's code to work
out whether it touches your disk is exactly the tax that makes people stop
reading and just run it.

`pyxray` parses the snippet, works out what it actually does, and draws that:

```
┌────────────────────────────────────────────────────────────────────────────────────┐
│ pyxray  sketchy.py                       22 lines · 16 statements · depth 2 · cx 2 │
│ makes 2 network calls → DELETES /tmp/stage → evaluates code at runtime → runs 2 …  │
│ risk ██████████████████  100  read it first                                        │
└────────────────────────────────────────────────────────────────────────────────────┘
┌─ C A P A B I L I T I E S ──────────────────────────────────────────────────────────┐
│  ≈ net 2!   » shell 2!   × delete 1!   ‡ eval 1!   § env 2   ▸ write 3   · print 1 │
└────────────────────────────────────────────────────────────────────────────────────┘
┌─ W H A T   I T   D O E S ──────────────────────────────────────────────────────────┐
│    4   § getenv   API_TOKEN                                                        │
│    7 ! × rmtree   /tmp/stage         ! recursively deletes a directory             │
│   10 ! ≈ GET      https://…/payload  ! verify=False — TLS certificates not checked │
│   12 ! ‡ unpickle pickle.loads       ! unpickling runs arbitrary code              │
│   17 ! » run      bash {name}.sh     ! shell=True — the argument goes through sh   │
└────────────────────────────────────────────────────────────────────────────────────┘
```

No execution, no sandbox, no imports resolved — it is a static read of the AST,
so it costs about two milliseconds and cannot itself do any of the things it
warns you about.

---

## Install

Needs a Rust toolchain (1.96+, for the Ruff parser) and Python 3.9+.

```sh
make release        # builds target/release/pyx
make install-ext    # puts the Python extension next to the package
```

On this machine, `source env.sh` first — it points `CARGO_HOME` and the target
directory at `/mnt/disk2` so the York home quota is left alone.

## Use it

```sh
pyx script.py                       # the dashboard
pyx --layout card script.py         # the glance
cat script.py | pyx                 # from a pipe
pyx --tui script.py                 # interactive; cycle themes and layouts live
pyx --format json script.py | jq    # the whole report as data
pyx --format svg -o x.svg script.py # for a document
```

Everything is themeable and every visual axis is a flag:

```sh
pyx --list                          # themes, layouts, formats
pyx -t carbon -l flow -i glyph script.py
pyx --contact-sheet sheet.html script.py   # every combination, one page
```

### In front of a command

```sh
pyx --exec --confirm script.py      # show the X-ray, then ask
pyx --exec --gate 40 script.py      # refuse outright above a risk of 40
```

### As a `python3` shim

```sh
source shell/pyxray.sh              # wraps python3; PYXRAY_OFF=1 to disable
export PYXRAY_GATE=60               # anything scarier stops and asks
```

### As a Claude Code hook

`.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "python3 -m pyxray.intercept --hook"
      }]
    }]
  }
}
```

Every Bash command carrying a Python heredoc gets X-rayed on the way past. Set
`PYXRAY_GATE` and anything above it turns into a permission prompt that says
why.

### From Python

```python
import pyxray

report = pyxray.analyze(source)
report["meta"]["synopsis"]      # 'reads 2 files → computes with pandas → writes 3 files'
report["metrics"]["risk"]       # 22
report["effects"][0]            # {'effect': 'fs_read', 'verb': 'read', 'target': 'data/in.csv', …}

print(pyxray.render(source, theme="carbon", layout="card", width=100))
```

---

## What it looks for

Effects are grouped into thirteen capabilities, each with a severity:

| | capability | flagged as caution when |
|---|---|---|
| `◂` | reads files | — |
| `▸` | writes files | — |
| `×` | deletes files | always |
| `≈` | network | POST/PUT/DELETE, `verify=False`, downloads to disk |
| `»` | runs commands | always; worse with `shell=True` |
| `§` | environment | reading a secret, `getpass` |
| `‡` | dynamic code | `eval`, `exec`, `pickle.loads`, `torch.load`, `yaml.load` |
| `·` | prints | — |
| `?` | randomness | — |
| `○` | time | — |
| `≡` | concurrency | — |
| `∑` | compute | — |
| `■` | exits | `os._exit` |

Resolution is more than name matching. Import aliases are unwound (`np.load` →
`numpy.load`); values are tracked well enough to resolve methods on them
(`fh = open(p)` … `fh.write(x)` is a write, and it names `p` as the target);
string constants are folded so `rmtree(STAGE)` shows `/tmp/stage` rather than
`STAGE`; `Path('out') / 'x.json'` stays a path; a function you defined called
`open` is not mistaken for the builtin; and shell fragments inside arguments
(`rm -rf`, `curl … | sh`, `sudo`) escalate whatever they are attached to.

The risk score is driven by severity and by *combinations* — fetching and then
executing scores higher than doing either twice.

## What it does not do

- It does not run anything, so anything decided at runtime is invisible to it.
  `getattr(os, "rem" + "ove")` will not be caught.
- It does not follow imports. A call into another module of your own is a call
  into a black box.
- A low score is not a safety claim. It is "nothing in this file's AST looks
  dangerous", which is a much smaller statement.

Treat it as a way to read faster, not as a way to stop reading.

---

## The pieces

```
crates/pyxray-core     parse → analyse → Report (serde; the stable contract)
crates/pyxray-render   Report → themed ratatui buffer → ANSI / text / HTML / SVG
crates/pyx-cli         the `pyx` binary and the interactive viewer
crates/pyxray-py       pyo3 bindings
python/pyxray          the Python package, the interceptor and the hook
attic/                 the first sketch of this, kept for reference
```

`pyxray-core` has no idea how anything is drawn, and `pyxray-render` has no
idea what Python is. A new look is a `Theme` value; a new arrangement is a
`Layout` arm; a new output format is a function over a ratatui `Buffer`.

Layouts are *planned* before they are drawn — `layout::plan` returns the exact
rectangle each panel will occupy — so measuring for a file and fitting into a
live terminal are the same code path and cannot drift apart.

## Looks

Seven themes, four layouts, three ways of labelling effects. `make sheet`
renders every combination against your own code so a look can be chosen by
comparison:

| theme | |
|---|---|
| `blueprint` | Technical drawing. Navy ground, cyan hairlines, letterspaced headings. |
| `neon` | After dark. Near-black ground, saturated magenta and lime, heavy rules. |
| `paper` | Daylight. Warm paper ground, ink type, rules instead of boxes. |
| `carbon` | Product UI. Neutral greys, one blue accent, rounded frames, tidy. |
| `amber` | Phosphor CRT. One hue, brightness does the work, bracketed panels. |
| `ansi` | Sixteen colours only. Survives any terminal, tmux and SSH session. |
| `mono` | No colour at all. Structure carried entirely by glyphs and weight. |

Every role in every colour theme clears 3:1 against its own background, and
body text runs 11:1 to 17.6:1.

| layout | |
|---|---|
| `card` | A glance: synopsis, risk, capabilities, effect timeline. |
| `dashboard` | Two columns: structure on the left, findings on the right. |
| `stack` | One column, everything, in reading order. |
| `flow` | The outline, given the room to breathe. |

`dashboard` falls back to `stack` below 96 columns rather than cramming.

## Numbers

| | |
|---|---|
| binary | 2.3 MB, no runtime dependencies |
| a 60-line snippet | ~5.4 ms end to end, of which ~3.2 ms is process startup |
| a 3600-line file | ~14 ms |
| tests | 38 (22 detection, 9 API, 6 render, 1 doc) |

## Licence

MIT.
