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

If `$HOME` is small or quota'd — cargo's registry plus a release build runs to
a couple of gigabytes — put them somewhere else and `source env.sh`:

```sh
export PYXRAY_BUILD_ROOT=/scratch/you/pyxray
source env.sh
```

`env.sh` also sources `env.local.sh` if you have one, which is gitignored.

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

### Watching an agent

An agent fires snippets several times a second, and that changes what the tool
has to be. Two things follow.

**The render is not for the model.** Whatever a hook or wrapper writes to
stdout or stderr is, in most harnesses, captured and fed back as tool output —
so drawing a report per snippet costs tokens, confuses the model, and scrolls
past you anyway. So by default pyxray appends one JSON line to a feed log and
draws nothing. You watch the log:

```sh
pyx watch                    # live column, in another pane
pyx watch --floor check      # only what wanted a second look
pyx watch --dump             # render it once and exit; scriptable
```

```
 pyxray watch   214 snippets  ·  38/min  ·  6 flagged            feed.jsonl
  risk ▁▁▂▁▁▁▂▁▁█▂▁▁▁▂▂▁▁▁▁▂▁
   seen        ▆▄▁ ▂▁▃ ▁ █▂▁▁ █▁  fs · world · eval · work · out
 ─────────────────────────────────────────────────────────────────────────
   12s █     1 ··· ··· · ···· ¶·  prints results                  <heredoc>
    9s █    30 ◂▸· ··§ · ∑··· ¶·  reads 2 files → writes 3 files  build.py
    4s ███ 100 ·▸× ≈»§ ‡ ···· ¶·  DELETES /tmp/stage → runs 2 cmd <heredoc>
                L7 rmtree /tmp/stage  recursively deletes a directory
    1s █    10 ··· ≈·· · ···· ··  calls https://pypi.org/…        <heredoc>
```

Every capability owns a permanent column, so the *pattern* is what you
recognise — the same profile always makes the same shape, and a snippet that
lights the delete slot is visible before a word is read. The header's histogram
sits in those same columns, so session totals line up with the rows underneath.

**Nothing is blocked unless you ask.** `PYXRAY_GATE` is unset by default and
pyxray only describes. Note that `PYXRAY_GATE=0` is not "no gate" — it refuses
anything with any effect at all. `off` is spelled out for that reason and is
the default.

The two defaults that matter, and why: interception uses `--layout auto`, so a
dull snippet gets one row and an alarming one blooms into the card and the
pane stays scannable through a burst; and the feed lands in
`$XDG_RUNTIME_DIR`, a tmpfs on any systemd machine, so a chatty session costs
no disk and the record dies with the login. Set `PYXRAY_LOG` if you want to
keep it.

### In front of a command you type

```sh
pyx --exec --confirm script.py      # show the X-ray, then ask
pyx --exec --gate 40 script.py      # refuse outright above a risk of 40
```

### As a PATH shim (works everywhere)

```sh
. integrations/shim/install.sh      # this shell
integrations/shim/install.sh --print >> ~/.bashrc
```

A real executable named `python3`, earlier on PATH than the real one. This is
the version that catches an agent, because it is resolved by `execvp` — by any
process, including the `sh -c` a harness spawns, a Makefile, or a
`subprocess.run` deep inside a library.

`shell/pyxray.sh` also defines a `python3` shell *function*, which is fine for
what you type yourself but will not catch an agent: non-interactive shells
never read your rc file, and many harnesses use `/bin/sh` rather than bash.

### As a harness hook

Per-harness setup lives in `integrations/`: Claude Code, Hermes, OpenCode, and
a tmux pane recipe. They all end up calling the same thing —

```sh
python3 -m pyxray.intercept --hook
```

— which reads a `PreToolUse`-shaped payload on stdin, finds the Python in it
(whether the tool takes Python directly or a shell command with a heredoc
inside), records it, and asks the harness to stop only if you set a gate.

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
| `¶` | prints | — |
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

The **band** is what the compact renders show, and it asks severity first, not
the score:

| band | when | reads as |
|---|---|---|
| inert | nothing notable | one faint cell |
| routine | notable work, any amount of it | one green cell |
| check it | anything at caution severity | two amber cells |
| read it first | caution, and a score at 60 or above | three red cells |

Severity leads deliberately. Volume must not be able to impersonate danger: a
script that writes three files is doing ordinary work and stays green at a
score of 30, while one `shutil.rmtree` is worth stopping for at the same score.
Colour and cell-count say the same thing twice, so the band survives a log
file, a screenshot, and a reader who does not see the hue.

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
