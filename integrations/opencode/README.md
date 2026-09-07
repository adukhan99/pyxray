# OpenCode

OpenCode loads plugins from `~/.config/opencode/plugins/` (global) or
`.opencode/plugins/` (project) at startup, and exposes `tool.execute.before`,
which fires before every tool call with the tool name and its arguments.

## Install

```sh
integrations/opencode/install.sh             # global
integrations/opencode/install.sh --project   # this project only
pyx watch                                    # in another pane
```

The installer bakes the absolute path to `integrations/pyxray-hook` into the
copied plugin, since a plugin in `~/.config` cannot find the checkout on its
own. `PYXRAY_HOOK` overrides it at runtime.

## What it does

Every `bash` command is checked for Python — a heredoc, a `-c` one-liner, a
script path — and recorded to the feed. Nothing is drawn into the OpenCode
session: at agent speed that would be unreadable, and OpenCode shows tool
output to the model. Watch `pyx watch` instead.

## Blocking

Throwing from `tool.execute.before` is how a plugin aborts a tool call, so
with `PYXRAY_GATE` set the plugin throws with pyxray's reason attached:

```
Error: pyxray: risk 100 is over the gate of 40 — DELETES /tmp/stage → runs 2 commands
```

Unset, which is the default, it never throws.

## Failure behaviour

If the hook cannot be spawned, or exits non-zero, or returns something that is
not JSON, the plugin stands down for the rest of the session rather than
failing on every command. An observer that breaks the agent's work is worse
than no observer, and OpenCode would surface a throwing hook as a tool error
on every single call.
