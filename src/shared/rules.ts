import { t } from "./i18n";
import type { AlwaysRule } from "./ipc";

/** Exactly what an Always rule allows from now on, in words. */
export function alwaysScope(rule: AlwaysRule): string {
  if (rule.tool_name === "Bash") return t("rules.every", rule.pattern);
  return t("rules.exact", rule.tool_name, rule.pattern);
}
