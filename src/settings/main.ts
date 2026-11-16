import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

import { button, el } from "../shared/dom";
import { diffView } from "../shared/diff-view";
import {
  api, HOOKS_EVENT, RULES_EVENT,
  type AlwaysRule, type HooksPreview, type HooksStatus,
} from "../shared/ipc";
import { alwaysScope } from "../shared/rules";

const root = document.getElementById("settings")!;
function section(title: string): HTMLElement {
  const section = el("section");
  const block = el("div", "card");
  section.append(el("h2", undefined, title), block);
  root.append(section);
  return block;
}
const hooksBlock = section("Claude Code");
const rulesBlock = section("Always rules");
const aboutBlock = section("About");
let status: HooksStatus | null = null;
let rules: AlwaysRule[] = [];
let busy = false;
let confirmationTimer: ReturnType<typeof setTimeout> | undefined;

type HookFlow =
  | { phase: "closed" }
  | { phase: "review"; preview: HooksPreview; removing: boolean }
  | { phase: "done"; backup: string | null }
  | { phase: "error"; message: string };
let flow: HookFlow = { phase: "closed" };

function closeFlow(): void {
  flow = { phase: "closed" };
  renderHooks();
}

function renderHooks(): void {
  hooksBlock.replaceChildren();
  if (status) {
    const row = el("div", "status-row");
    row.append(
      el("span", status.installed ? "status-dot connected" : "status-dot"),
      el("span", undefined, status.installed ? "Connected" : "Not connected"),
    );
    hooksBlock.append(row, el("div", "note note-dim", status.settings_path));
  }
  const actions = el("div", "actions");
  if (flow.phase === "closed") {
    if (status) {
      actions.append(button("btn-neutral", status.installed ? "Remove hooks…" : "Install hooks…", () => void previewHooks()));
    }
  } else if (flow.phase === "review") {
    const { preview, removing } = flow;
    if (preview.changes) {
      hooksBlock.append(
        el(
          "div",
          "note",
          `Cortex will ${removing ? "remove its hooks from" : "add its hooks to"} this file. A dated backup is saved first; nothing else in the file changes.`,
        ),
        diffView(preview.diff),
      );
      actions.append(
        button("btn-neutral", "Cancel", closeFlow),
        button(removing ? "btn-deny" : "btn-allow", removing ? "Remove hooks" : "Install hooks", () => void confirmHooks(preview, removing)),
      );
    } else {
      hooksBlock.append(el("div", "note", "Nothing to change."));
      actions.append(button("btn-neutral", "Close", closeFlow));
    }
  } else {
    if (flow.phase === "error") {
      hooksBlock.append(el("div", "note note-error", flow.message));
    } else {
      hooksBlock.append(el("div", "note", "Done."));
      if (flow.backup) hooksBlock.append(el("div", "note note-dim", `Backup: ${flow.backup}`));
    }
    actions.append(button("btn-neutral", "Close", closeFlow));
  }
  actions.querySelectorAll("button").forEach((node) => { node.disabled = busy; });
  hooksBlock.append(actions);
}

async function previewHooks(): Promise<void> {
  if (!status || busy) return;
  const removing = status.installed;
  busy = true;
  renderHooks();
  try {
    const preview = await (removing ? api.hooksUninstallPreview() : api.hooksPreview());
    flow = { phase: "review", preview, removing };
  } catch (error) {
    flow = { phase: "error", message: String(error) };
  } finally {
    busy = false;
    renderHooks();
  }
}

async function confirmHooks(preview: HooksPreview, removing: boolean): Promise<void> {
  if (busy) return;
  busy = true;
  renderHooks();
  try {
    const result = await (removing ? api.hooksUninstall(preview.token) : api.hooksInstall(preview.token));
    flow = { phase: "done", backup: result.backup_path };
    status = await api.hooksStatus();
  } catch (error) {
    flow = { phase: "error", message: String(error) };
  } finally {
    busy = false;
    renderHooks();
  }
}

function clearConfirmation(): void {
  if (confirmationTimer !== undefined) clearTimeout(confirmationTimer);
  confirmationTimer = undefined;
}

async function changeRules(change: () => Promise<void>): Promise<void> {
  try {
    await change();
  } catch (error) {
    renderRules();
    rulesBlock.append(el("div", "note note-error", String(error)));
  }
}

function renderRules(): void {
  clearConfirmation();
  rulesBlock.replaceChildren(el("div", "note", 'Requests matching a rule are allowed without asking. Rules are created by "Always" on an approval card and stored only on this computer.'));
  if (rules.length === 0) {
    rulesBlock.append(el("div", "note empty", "No rules yet."));
    return;
  }
  for (const rule of rules) {
    const row = el("div", "rule-row");
    row.append(el("span", "rule-scope", alwaysScope(rule)), button("btn-neutral", "Remove", () => void changeRules(() => api.ruleRemove(rule))));
    rulesBlock.append(row);
  }
  const actions = el("div", "actions");
  const removeAll = button("btn-deny", "Remove all", () => {
    if (confirmationTimer !== undefined) {
      clearConfirmation();
      renderRules();
      void changeRules(() => api.rulesClear());
      return;
    }
    removeAll.textContent = `Click again to remove all ${rules.length} rules`;
    confirmationTimer = setTimeout(() => {
      clearConfirmation();
      renderRules();
    }, 4000);
  });
  actions.append(removeAll);
  rulesBlock.append(actions);
}

async function start(): Promise<void> {
  // Subscribe before loading; an event received during a load is fresher.
  let hooksChanged = false;
  let rulesChanged = false;
  await listen<HooksStatus>(HOOKS_EVENT, (event) => {
    hooksChanged = true;
    status = event.payload;
    renderHooks();
  });
  await listen<AlwaysRule[]>(RULES_EVENT, (event) => {
    rulesChanged = true;
    rules = event.payload;
    renderRules();
  });
  await Promise.all([
    api.hooksStatus().then((value) => { if (!hooksChanged) status = value; }).catch((error: unknown) => {
      flow = { phase: "error", message: String(error) };
    }).finally(renderHooks),
    api.rulesList().then((value) => { if (!rulesChanged) rules = value; renderRules(); }).catch((error: unknown) => {
      rulesBlock.append(el("div", "note note-error", String(error)));
    }),
    getVersion().then((version) => { aboutBlock.prepend(el("div", undefined, `Cortex ${version}`)); }).catch((error: unknown) => {
      aboutBlock.prepend(el("div", "note note-error", String(error)));
    }),
  ]);
}

aboutBlock.append(
  el("div", "note", "Local by design: no account, no telemetry."),
  button("btn-neutral", "Quit Cortex", () => void api.quit()),
);
window.addEventListener("pagehide", clearConfirmation);
void start();
