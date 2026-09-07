# Integrations

Every integration here ends up calling one executable:

```sh
integrations/pyxray-hook
```

It reads a `PreToolUse`-shaped JSON payload on stdin, finds the Python inside
it, appends a row to the feed, and writes a block decision on stdout **only**
if `PYXRAY_GATE` is set. Ungated it prints nothing and exits 0. It resolves its
own location, so nothing has to be told where pyxray lives, and it swallows its
own failures — a hook that errors blocks the tool call, and an observer that
breaks somebody else's work is worse than no observer.

## Which surface do I want?

| you are | use | why |
|---|---|---|
| typing commands yourself | `shell/pyxray.sh` | a shell function is enough |
| running an agent | that harness's hook, below | the harness knows what it is about to run |
| running an agent with no hook API | `integrations/shim` | a PATH shim catches anything that spawns a process |
| running something exotic | `integrations/shim` | same reason |

**A shell function will not catch an agent.** Harnesses spawn non-interactive
shells, which never read your rc file, and many use `/bin/sh` rather than bash.
A PATH shim is a real executable resolved by `execvp`, so it catches the
agent's `sh -c`, a Makefile, and a `subprocess.run` deep inside some library.

## Harnesses

| | how | notes |
|---|---|---|
| [Claude Code](claude-code/) | plugin, or a `PreToolUse` hook in `settings.json` | Python arrives inside `Bash` commands, usually as a heredoc |
| [Hermes](hermes/) | `hooks:` block in `~/.hermes/config.yaml` | two surfaces: `execute_code` takes Python directly, `terminal` takes a shell command |
| [OpenCode](opencode/) | plugin on `tool.execute.before` | blocks by throwing |
| anything else | [PATH shim](shim/) | no API needed |

## A note on models and harnesses

There is nothing to integrate with a *model*. DeepSeek, Claude, GPT, Qwen and
the rest are things a harness talks to; they have no plugin surface and never
see your filesystem. What runs the Python is the harness around the model, and
that is what pyxray hooks. An agent pointed at DeepSeek is covered by whichever
harness it is — OpenCode, Hermes, something homegrown — or, failing all of
those, by the PATH shim, which does not care what is upstream.

This also means pyxray gets more useful, not less, as models get better.
Frontier models are not going to `rm -rf` your home directory. What they do
constantly is run twenty snippets a minute that each touch a few files, and the
question worth answering is not "is this one malicious" but "what has all of
this been doing". That is why the default is a feed and not a blocker.

## Watching

```sh
pyx watch                       # live column
pyx watch --floor check         # only what wanted a second look
pyx watch --dump                # render once and exit; for scripts, status bars
integrations/tmux/pyxray-pane.sh    # open it in a tmux split
```

## Configuration

All of it is environment variables, so it works the same through every
integration.

| | |
|---|---|
| `PYXRAY_OFF=1` | do nothing at all |
| `PYXRAY_LOG` | feed log path (default: `$XDG_RUNTIME_DIR/pyxray/feed.jsonl`) |
| `PYXRAY_DRAW` | `never` \| `tty` \| `always` (default: `tty`) |
| `PYXRAY_TTY` | draw to this device instead, e.g. `/dev/pts/7` |
| `PYXRAY_LAYOUT` | `line` \| `auto` \| `card` \| `dashboard` \| `stack` \| `flow` |
| `PYXRAY_THEME` | `blueprint` \| `neon` \| `paper` \| `carbon` \| `amber` \| `ansi` \| `mono` |
| `PYXRAY_ICONS` | `glyph` \| `tag` \| `both` |
| `PYXRAY_WIDTH` | columns |
| `PYXRAY_GATE` | `off` (default), or a risk score to refuse above |
| `PYXRAY_CONFIRM=1` | ask before running (shim only; a hook has no terminal) |
| `PYXRAY_MIN_LINES` | ignore snippets shorter than this |

`PYXRAY_GATE=0` is **not** "no gate" — it refuses anything with any effect at
all. `off` is spelled out for exactly that reason.
