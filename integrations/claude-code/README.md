# Claude Code

Claude Code runs Python through the `Bash` tool, almost always as a heredoc:

```
python3 <<'EOF'
...
EOF
```

so the hook matches `Bash` and pulls the Python back out of the command.

## As a plugin

```sh
# from a marketplace, or straight from the checkout:
/plugin install ./integrations/claude-code
```

The plugin ships `hooks/hooks.json`, which points at
`integrations/pyxray-hook` via `${CLAUDE_PLUGIN_ROOT}`. Nothing else to set.

## Without the plugin

Add this to `.claude/settings.json` (project) or `~/.claude/settings.json`
(everywhere), with the absolute path to your checkout:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "/path/to/pyxray/integrations/pyxray-hook",
        "timeout": 5
      }]
    }]
  }
}
```

`integrations/claude-code/install.sh` will write that for you.

## Then

```sh
pyx watch
```

in a second terminal, and every snippet Claude runs lands there as one row.

The hook prints nothing unless you set `PYXRAY_GATE`. When you do, anything
above it comes back as `permissionDecision: "ask"` with the reason attached,
so Claude Code prompts you rather than silently refusing — the model still
sees why, and you still decide.

## Notes

- Claude Code hooks receive `{"tool_name": "Bash", "tool_input": {"command": ...}}`
  on stdin. The hook ignores anything with no Python in it and exits 0.
- A hook that errors would block the tool call, so `pyxray-hook` swallows its
  own failures. If it seems inert, run it by hand:
  `echo '{"tool_name":"Bash","tool_input":{"command":"python3 -c \"import os\""}}' | integrations/pyxray-hook`
- Set `PYXRAY_DRAW=never` (the default) so the render never reaches the
  transcript. Use `pyx watch`, or point `PYXRAY_TTY` at a spare terminal.
