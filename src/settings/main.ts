import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

import { button, el } from "../shared/dom";
import { diffView } from "../shared/diff-view";
import {
  api, HOOKS_EVENT, RULES_EVENT, GITHUB_EVENT, GITHUB_CLI_URL, GITHUB_DEVICE_URL, releaseNotesUrl,
  type GithubStatus, UPDATES_EVENT, type UpdateStatus, ISLAND_PREFS_EVENT, type IslandPrefs, type AutostartStatus,
  ISLAND_SCREENS_EVENT, type IslandPosition, type IslandScreen,
  type AlwaysRule, type HooksPreview, type HooksStatus,
} from "../shared/ipc";
import { MASCOT_NAMES, MASCOTS, mascotSvg } from "../shared/mascots";
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
const islandBlock = section("Island");
const POSITIONS: Record<IslandPosition, string> = {
  top: "Top center",
  left: "Left side",
  right: "Right side",
  "bottom-left": "Bottom left",
  "bottom-right": "Bottom right",
};
const startupBlock = section("Startup");
const updatesBlock = section("Updates");
const aboutBlock = section("About");
let status: HooksStatus | null = null;
let rules: AlwaysRule[] = [];
let busy = false;
let github: GithubStatus | null = null;
let githubBusy = false;
let githubRevision = 0;
let githubError: string | null = null;
let updates: UpdateStatus | null = null;
let updatesBusy = false;
let updatesError: string | null = null;
let island: IslandPrefs | null = null;
let startup: AutostartStatus | null = null;
let startupError: string | null = null;
let startupBusy = false;
let islandError: string | null = null;
let islandBusy = false;
let screens: IslandScreen[] | null = null;
let screensRevision = 0;
let screensError: string | null = null;
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

// Kept as state: the rules event of the same change can arrive after the
// error and redraw the block.
let rulesError: string | null = null;

async function changeRules(change: () => Promise<void>): Promise<void> {
  rulesError = null;
  try {
    await change();
  } catch (error) {
    rulesError = String(error);
  }
  renderRules();
}

function renderRules(): void {
  clearConfirmation();
  rulesBlock.replaceChildren(el("div", "note", 'Requests matching a rule are allowed without asking. Rules are created by "Always" on an approval card and stored only on this computer.'));
  if (rulesError) rulesBlock.append(el("div", "note note-error", rulesError));
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
  githubBlock.replaceChildren(
    el("div", "note", "Shows your open pull requests and the reviews waiting for you, using the GitHub CLI (gh) you are already signed in to. Cortex never stores a GitHub token. Checks every 3 minutes while on."),
    el("div", "note", "Every account signed in to gh is shown; hide the ones you don't want."),
  );
  const action = (style: string, label: string, change: () => Promise<void>, disabled = false): HTMLButtonElement => {
    const node = button(style, label, () => void changeGithub(change));
    node.disabled = githubBusy || disabled;
    return node;
  };
  if (github) {
    const { accounts, enabled, installer, state, task } = github;
    const running = task.kind === "installing" || task.kind === "signing_in";
    const labels = {
      off: "Off",
      checking: accounts.length > 0 ? "On" : "Checking…",
      ok: "On",
      no_gh: "GitHub CLI not found",
      signed_out: "Signed out",
      error: github.message ?? "GitHub request failed",
    };
    const row = el("div", "status-row");
    row.append(el("span", state === "ok" ? "status-dot connected" : "status-dot"), el("span", undefined, labels[state]));
    githubBlock.append(row);

    const actions = el("div", "actions");
    if (enabled) {
      for (const account of accounts) {
        const row = el("div", "rule-row");
        const label = el("div", "rule-scope");
        label.append(el("span", undefined, account.login));
        if (account.active) label.append(el("span", "note", " · active in gh"));
        if (account.message) label.append(el("div", "note", account.message));
        row.append(
          el("span", account.state === "ok" ? "status-dot connected" : `status-dot github-status-${account.state}`),
          label,
          action("btn-neutral", account.shown ? "Hide" : "Show", () => api.githubSetAccountShown(account.login, !account.shown)),
        );
        githubBlock.append(row);
      }
      if (state === "no_gh") {
        if (installer) {
          actions.append(action("btn-allow", "Install GitHub CLI", api.githubInstall, running));
        } else {
          actions.append(action("btn-neutral", "Get GitHub CLI", () => api.openUrl(GITHUB_CLI_URL)));
          if (/Linux/i.test(navigator.userAgent)) {
            githubBlock.append(el("div", "note", "Install it with your package manager, then run gh auth login."));
          }
        }
      } else if (accounts.length > 0 || state === "signed_out") {
        actions.append(action("btn-allow", state === "signed_out" ? "Sign in to GitHub" : "Add account", api.githubSignIn, running));
      }
      actions.append(action("btn-neutral", "Refresh now", api.githubRefresh));
    }
    actions.append(action(enabled ? "btn-neutral" : "btn-allow", enabled ? "Turn off" : "Turn on", () => api.githubSetEnabled(!enabled)));
    githubBlock.append(actions);

    if (task.kind === "installing" || task.kind === "signing_in") {
      const area = el("div", "status-row github-task");
      if (task.kind === "installing") {
        area.append(el("span", undefined, "Installing GitHub CLI…"));
      } else if (task.code) {
        area.append(
          el("span", undefined, "Enter this code on GitHub:"),
          el("code", "github-device-code", task.code),
          action("btn-allow", "Open GitHub", () => api.openUrl(GITHUB_DEVICE_URL)),
        );
      } else {
        area.append(
          el("span", undefined, "Starting sign-in…"),
          el("span", "note", "If no code appears, cancel and run gh auth login in a terminal."),
        );
      }
      area.append(action("btn-neutral", "Cancel", api.githubCancelTask));
      githubBlock.append(area);
    } else if (task.kind === "failed") {
      githubBlock.append(el("div", "note note-error", task.message));
    }
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

function renderUpdates(): void {
  updatesBlock.replaceChildren(
    el("div", "note", "When on, Cortex asks GitHub for a new release at launch and every 6 hours, and sends nothing else. An update is installed only if it carries Cortex's signature, and only when you click Update and restart."),
  );
  if (updates) {
    const { enabled, state, version, message } = updates;
    const labels = {
      unsupported: "Not available for this package: update it with your package manager.",
      off: "Off",
      idle: "On · up to date",
      checking: "Checking…",
      downloading: `Downloading Cortex ${version ?? ""}…`,
      ready: `Cortex ${version ?? ""} is ready`,
      error: message ?? "Update check failed",
    };
    const row = el("div", "status-row");
    row.append(el("span", state === "idle" || state === "ready" ? "status-dot connected" : "status-dot"), el("span", undefined, labels[state]));
    updatesBlock.append(row);
    if (state !== "unsupported") {
      const actions = el("div", "actions");
      if (state === "ready") actions.append(button("btn-allow", "Update and restart", () => void changeUpdates(api.updatesInstall)));
      const on = enabled === true;
      actions.append(button(on ? "btn-neutral" : "btn-allow", on ? "Turn off" : "Turn on", () => void changeUpdates(() => api.updatesSetEnabled(!on))));
      actions.querySelectorAll("button").forEach((node) => { node.disabled = updatesBusy; });
      updatesBlock.append(actions);
    }
  }
  if (updatesError) updatesBlock.append(el("div", "note note-error", updatesError));
}
async function changeUpdates(change: () => Promise<void>): Promise<void> {
  if (updatesBusy) return;
  updatesBusy = true;
  updatesError = null;
  renderUpdates();
  try {
    await change();
  } catch (error) {
    updatesError = String(error);
  } finally {
    updatesBusy = false;
    renderUpdates();
  }
}

function islandChoice(label: string, selected: boolean, change?: () => Promise<void>): HTMLButtonElement {
  const choice = button(selected ? "mascot-choice selected" : "mascot-choice", label, () => {
    if (change) void changeIsland(change);
  });
  choice.setAttribute("aria-pressed", String(selected));
  choice.disabled = islandBusy || !change;
  return choice;
}

function renderIsland(): void {
  islandBlock.replaceChildren(el("div", "note", "The mascot sits in the island and takes the color of what needs you: green when an agent is done, amber when it waits for you, red when something failed."));
  const prefs = island;
  if (prefs) {
    const gallery = el("div", "mascot-gallery");
    for (const mascot of MASCOTS) {
      const choice = islandChoice("", mascot === prefs.mascot, () => api.islandSetMascot(mascot));
      choice.title = MASCOT_NAMES[mascot];
      choice.append(mascotSvg(mascot) ?? el("span", "mascot-none"), el("span", "mascot-name", MASCOT_NAMES[mascot]));
      gallery.append(choice);
    }

    const positions = el("div", "mascot-gallery");
    for (const position of Object.keys(POSITIONS) as IslandPosition[]) {
      positions.append(islandChoice(POSITIONS[position], position === prefs.position, () => api.islandSetPosition(position)));
    }

    const displays = el("div", "mascot-gallery screen-gallery");
    displays.append(islandChoice("Main screen", prefs.screen === null, () => api.islandSetScreen(null)));
    for (const screen of screens ?? []) {
      displays.append(islandChoice(
        `${screen.name} · ${screen.width} × ${screen.height}${screen.primary ? " · main" : ""}`,
        screen.name === prefs.screen,
        () => api.islandSetScreen(screen.name),
      ));
    }
    if (screens && prefs.screen !== null && !screens.some(({ name }) => name === prefs.screen)) {
      displays.append(islandChoice(`${prefs.screen} (not connected)`, true));
    }
    islandBlock.append(gallery, el("h3", undefined, "Position"), positions, el("h3", undefined, "Screen"), displays);
  }
  if (islandError) islandBlock.append(el("div", "note note-error", islandError));
  if (screensError) islandBlock.append(el("div", "note note-error", screensError));
}

async function changeIsland(change: () => Promise<void>): Promise<void> {
  if (islandBusy) return;
  islandBusy = true;
  islandError = null;
  renderIsland();
  try {
    await change();
  } catch (error) {
    islandError = String(error);
  } finally {
    islandBusy = false;
    renderIsland();
  }
}

async function refreshScreens(): Promise<void> {
  const revision = ++screensRevision;
  try {
    const connected = await api.islandScreens();
    if (revision !== screensRevision) return;
    screens = connected;
    screensError = null;
  } catch (error) {
    if (revision !== screensRevision) return;
    screensError = String(error);
  }
  renderIsland();
}

function renderStartup(): void {
  startupBlock.replaceChildren(el("div", "note", "Cortex opens when you log in, straight into the island, with no opening screen."));
  if (startup) {
    const row = el("div", "status-row");
    const label = !startup.available ? "Not available for this copy: open Cortex from where it is installed." : startup.enabled ? "On · opens at login" : "Off";
    row.append(el("span", startup.enabled ? "status-dot connected" : "status-dot"), el("span", undefined, label));
    startupBlock.append(row);
    if (startup.available || startup.enabled) {
      const on = startup.enabled;
      const toggle = button(on ? "btn-neutral" : "btn-allow", on ? "Turn off" : "Turn on", () => void setStartup(!on));
      toggle.disabled = startupBusy;
      startupBlock.append(toggle);
    }
  }
  if (startupError) startupBlock.append(el("div", "note note-error", startupError));
}
async function setStartup(enabled: boolean): Promise<void> {
  if (startupBusy) return;
  startupBusy = true;
  renderStartup();
  try {
    startup = await api.autostartSet(enabled);
    startupError = null;
  } catch (error) {
    startupError = String(error);
  }
  startupBusy = false;
  renderStartup();
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
  let updatesChanged = false;
  await listen<UpdateStatus>(UPDATES_EVENT, (event) => {
    updatesChanged = true;
    updates = event.payload;
    renderUpdates();
  });
  let islandChanged = false;
  await listen<IslandPrefs>(ISLAND_PREFS_EVENT, (event) => {
    islandChanged = true;
    island = event.payload;
    renderIsland();
  });
  await listen(ISLAND_SCREENS_EVENT, () => void refreshScreens());
  // Only macOS reports a screen change; elsewhere the list is read again
  // whenever Settings comes forward.
  window.addEventListener("focus", () => void refreshScreens());
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
    refreshScreens(),
    api.autostartStatus().then((value) => { startup = value; }).catch((error: unknown) => { startupError = String(error); }).finally(renderStartup),
    api.islandPrefs().then((value) => { if (!islandChanged) island = value; }).catch((error: unknown) => { islandError = String(error); }).finally(renderIsland),
    api.updatesStatus().then((value) => { if (!updatesChanged) updates = value; }).catch((error: unknown) => { updatesError = String(error); }).finally(renderUpdates),
    api.githubStatus().then((value) => { if (!githubChanged) github = value; }).catch((error: unknown) => { githubError = String(error); }).finally(renderGithub),
    api.hooksStatus().then((value) => { if (!hooksChanged) status = value; }).catch((error: unknown) => {
      flow = { phase: "error", message: String(error) };
    }).finally(renderHooks),
    api.rulesList().then((value) => { if (!rulesChanged) rules = value; renderRules(); }).catch((error: unknown) => {
      rulesBlock.append(el("div", "note note-error", String(error)));
    }),
    getVersion().then((version) => {
      const commit = import.meta.env.VITE_CORTEX_COMMIT;
      aboutBlock.prepend(el("div", undefined, commit ? `Cortex ${version} · test build ${commit}` : `Cortex ${version}`));
      aboutActions.prepend(button("btn-neutral", "What's new", () => {
        api.openUrl(releaseNotesUrl(version)).catch((error: unknown) => { aboutBlock.append(el("div", "note note-error", String(error))); });
      }));
    }).catch((error: unknown) => {
      aboutBlock.prepend(el("div", "note note-error", String(error)));
    }),
  ]);
}

const aboutActions = el("div", "actions");
aboutActions.append(button("btn-neutral", "Quit Cortex", () => void api.quit()));
aboutBlock.append(el("div", "note", "Local by design: no account, no telemetry."), aboutActions);
window.addEventListener("pagehide", clearConfirmation);
void start();
