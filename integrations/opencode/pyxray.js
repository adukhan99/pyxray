/**
 * pyxray for OpenCode.
 *
 * Every bash command the agent runs is checked for Python on the way past. The
 * analysis is appended to the pyxray feed — watch it with `pyx watch` in
 * another pane — and the command is only ever stopped if PYXRAY_GATE is set.
 *
 * Install: integrations/opencode/install.sh
 * By hand: copy to ~/.config/opencode/plugins/ (global) or .opencode/plugins/
 *          (project), and set PYXRAY_HOOK to the absolute path of
 *          integrations/pyxray-hook.
 */

import { spawnSync } from "node:child_process";

// install.sh rewrites this line with an absolute path.
const HOOK = process.env.PYXRAY_HOOK || "PYXRAY_HOOK_PATH";

/** Tools whose arguments might carry Python. Everything else is ignored. */
const WATCHED = new Set([
  "bash",
  "shell",
  "terminal",
  "run_command",
  "python",
  "execute_code",
]);

export const PyxrayPlugin = async () => {
  let broken = false;

  return {
    "tool.execute.before": async (input, output) => {
      if (broken || process.env.PYXRAY_OFF === "1") return;
      if (!WATCHED.has(String(input.tool || "").toLowerCase())) return;

      const payload = JSON.stringify({
        hook_event_name: "PreToolUse",
        tool_name: input.tool,
        tool_input: output.args ?? {},
        session_id: input.sessionID ?? input.sessionId ?? "",
        hook_source: "opencode",
      });

      let result;
      try {
        result = spawnSync(HOOK, [], {
          input: payload,
          encoding: "utf8",
          timeout: 5000,
        });
      } catch {
        // A plugin that throws here would abort the tool call, and an observer
        // that breaks the agent's work is worse than no observer. Stand down
        // for the rest of the session rather than fail on every command.
        broken = true;
        return;
      }
      if (!result || result.error || result.status !== 0) {
        broken = true;
        return;
      }

      const text = (result.stdout || "").trim();
      if (!text) return; // nothing to say — the common case

      let decision;
      try {
        decision = JSON.parse(text);
      } catch {
        return;
      }
      if (decision && decision.decision === "block") {
        // Throwing is how a tool.execute.before hook aborts a call in
        // OpenCode. Only reachable when PYXRAY_GATE is set.
        throw new Error(decision.reason || "pyxray: blocked by gate");
      }
    },
  };
};

export default PyxrayPlugin;
