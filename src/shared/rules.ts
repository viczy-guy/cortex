import type { AlwaysRule } from "./ipc";

/** Exactly what an Always rule allows from now on, in words. */
export function alwaysScope(rule: AlwaysRule): string {
  if (rule.tool_name === "Bash") return `Every "${rule.pattern}" command, without asking`;
  return `${rule.tool_name} on exactly ${rule.pattern}, without asking`;
}
