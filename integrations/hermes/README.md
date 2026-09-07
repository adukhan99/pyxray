# Hermes

Hermes has two surfaces that run Python, and the hook covers both:

| tool | argument | what arrives |
|---|---|---|
| `execute_code` | `code` | Python, directly |
| `terminal` | `command` | a shell command, possibly with a heredoc in it |

## Install

```sh
integrations/hermes/install.sh            # print the block to paste
integrations/hermes/install.sh --append   # append it, if that is safe
```

Then, once:

```sh
export HERMES_ACCEPT_HOOKS=1    # or run hermes with --accept-hooks
pyx watch                       # in another pane
```

## How it hangs together

Hermes' shell-hook bridge reads a `hooks:` block from `~/.hermes/config.yaml`
and registers each entry on the same plugin hook manager its Python plugins
use, so a shell hook and a Python plugin compose rather than compete. Python
plugins register first, so their block decisions win ties.

The wire protocol is deliberately Claude-Code-shaped, which is why one script
serves both harnesses. Hermes accepts either spelling of a block:

```json
{"decision": "block", "reason":  "..."}
{"action":   "block", "message": "..."}
```

`pyxray-hook` emits the first, plus Claude Code's `hookSpecificOutput`, and
only when `PYXRAY_GATE` is set. Ungated it prints nothing at all.

## Consent

Hermes prompts for consent on first use of each `(event, command)` pair and
records the answer in `~/.hermes/shell-hooks-allowlist.json`. A non-TTY caller
has to promise consent up front, via `--accept-hooks`, `HERMES_ACCEPT_HOOKS=1`,
or `hooks_auto_accept: true` in the config — otherwise registration is
declined and the hook simply never fires.

`HERMES_SAFE_MODE=1` disables plugins, MCP and hooks together, which is the
right escape hatch if you ever need to rule pyxray out.

## Notes

- The bridge's own docstring calls the file `cli-config.yaml`; the loader
  actually reads `~/.hermes/config.yaml`. `cli-config.yaml.example` in the
  Hermes tree is an example, not the live path.
- `matcher` is a regex over the tool name and is honoured only for
  `pre_tool_call` and `post_tool_call`. On any other event Hermes warns and
  ignores it.
- Commands run through `shlex.split` with `shell=False`, so pipes and
  redirection do not work — wrap anything like that in a script, which is what
  `integrations/pyxray-hook` already is.
- `timeout` is clamped; 5s is far more than pyxray needs (a 3600-line file
  analyses in about 14ms).
- The installer honours `HERMES_HOME`, which matters if your Hermes tree is
  not at `~/.hermes` — check with `echo $HERMES_HOME` before running it.
