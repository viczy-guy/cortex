import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

import { button, el } from "../shared/dom";
import { diffView } from "../shared/diff-view";
import {
  api, HOOKS_EVENT, RULES_EVENT, GITHUB_EVENT,
  type GithubStatus,
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
const githubBlock = section("GitHub");
const aboutBlock = section("About");
let status: HooksStatus | null = null;
let rules: AlwaysRule[] = [];
let busy = false;
let github: GithubStatus | null = null;
let githubBusy = false;
let githubRevision = 0;
let githubError: string | null = null;
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
      el("span", undefined, status.outdated ? "Connected · update available" : status.installed ? "Connected" : "Not connected"),
    );
    hooksBlock.append(row, el("div", "note note-dim", status.settings_path));
  }
  const actions = el("div", "actions");
  if (flow.phase === "closed") {
    if (status) {
      const installed = status.installed;
      if (status.outdated) actions.append(button("btn-allow", "Update hooks…", () => void previewHooks(false)));
      actions.append(button("btn-neutral", installed ? "Remove hooks…" : "Install hooks…", () => void previewHooks(installed)));
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

async function previewHooks(removing: boolean): Promise<void> {
  if (!status || busy) return;
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

function renderGithub(): void {
  githubBlock.replaceChildren(el("div", "note", "Shows your open pull requests and the reviews waiting for you, using the GitHub CLI (gh) you are already signed in to. Cortex never stores a GitHub token. Checks every 3 minutes while on."));
  if (github) {
    const labels = {
      off: "Off",
      // A periodic re-check keeps showing the account it already has.
      checking: github.summary ? `On · signed in as ${github.summary.login}` : "Checking…",
      ok: `On · signed in as ${github.summary?.login ?? ""}`,
      no_gh: "GitHub CLI not found — install it from cli.github.com",
      signed_out: "Signed out — run `gh auth login` in a terminal",
      error: github.message ?? "GitHub request failed",
    };
    const row = el("div", "status-row");
    row.append(el("span", github.state === "ok" ? "status-dot connected" : "status-dot"), el("span", undefined, labels[github.state]));
    const actions = el("div", "actions");
    const enabled = github.enabled;
    actions.append(button(enabled ? "btn-neutral" : "btn-allow", enabled ? "Turn off" : "Turn on", () => void changeGithub(() => api.githubSetEnabled(!enabled))));
    if (enabled) actions.append(button("btn-neutral", "Refresh now", () => void changeGithub(api.githubRefresh)));
    actions.querySelectorAll("button").forEach((node) => { node.disabled = githubBusy; });
    githubBlock.append(row, actions);
  }
  if (githubError) githubBlock.append(el("div", "note note-error", githubError));
}
async function changeGithub(change: () => Promise<void>): Promise<void> {
  if (githubBusy) return;
  githubBusy = true;
  githubError = null;
  renderGithub();
  try {
    await change();
    const revision = githubRevision;
    const value = await api.githubStatus();
    if (githubRevision === revision) github = value;
  } catch (error) {
    githubError = String(error);
  } finally {
    githubBusy = false;
    renderGithub();
  }
}

async function start(): Promise<void> {
  // Subscribe before loading; an event received during a load is fresher.
  let githubChanged = false;
  await listen<GithubStatus>(GITHUB_EVENT, (event) => {
    githubChanged = true;
    githubRevision++;
    github = event.payload;
    renderGithub();
  });
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
    api.githubStatus().then((value) => { if (!githubChanged) github = value; }).catch((error: unknown) => { githubError = String(error); }).finally(renderGithub),
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
