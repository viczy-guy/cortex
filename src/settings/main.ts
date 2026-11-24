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
import { playSound } from "../shared/sounds";

// A portal (#151): one page per subject, picked in the sidebar.
const nav = document.getElementById("settings-nav")!;
const root = document.getElementById("settings")!;
type PageId = "overview" | "agents" | "island" | "notices" | "github" | "general" | "about";
const pages = new Map<PageId, { page: HTMLElement; item: HTMLButtonElement }>();

/** Stroked 16 × 16 outlines, one path each. */
const ICONS: Record<PageId, string> = {
  overview: "M2 2h5v5H2zM9 2h5v5H9zM2 9h5v5H2zM9 9h5v5H9z",
  agents: "M1.5 2.5h13v11h-13zM4.5 6.5l2 1.5-2 1.5M8.5 10h3",
  island: "M2 3h12v1.5a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3zM6.5 5.3v.1M9.5 5.3v.1M3 12h10",
  notices: "M4 11V7.5a4 4 0 0 1 8 0V11l1.2 1.5H2.8zM6.6 14a1.5 1.5 0 0 0 2.8 0",
  github: "M3 3.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0M3 12.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0M10 12.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0M4.5 5v6M11.5 11V7a2 2 0 0 0-2-2H8m1.5-1.8L7.7 5l1.8 1.8",
  general: "M5.8 8a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M3.6 12.4L5 11M11 5l1.4-1.4",
  about: "M1.7 8a6.3 6.3 0 1 0 12.6 0a6.3 6.3 0 1 0-12.6 0M8 7.2v4M8 4.8v.1",
};

function navIcon(id: PageId): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("class", "nav-icon");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", ICONS[id]);
  svg.append(path);
  return svg;
}

function page(id: PageId, name: string): HTMLElement {
  const node = el("section", "page");
  node.hidden = true;
  node.append(el("h1", undefined, name));
  root.append(node);
  const item = el("button", "nav-item");
  item.append(navIcon(id), el("span", undefined, name));
  item.addEventListener("click", () => show(id));
  nav.append(item);
  pages.set(id, { page: node, item });
  return node;
}

function show(id: PageId): void {
  for (const [key, { page, item }] of pages) {
    page.hidden = key !== id;
    if (key === id) item.setAttribute("aria-current", "page");
    else item.removeAttribute("aria-current");
  }
  root.scrollTop = 0;
}

nav.addEventListener("keydown", (event) => {
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  const ids = [...pages.keys()];
  // From the focused item: Tab moves the focus without opening its page.
  const from = ids.findIndex((id) => pages.get(id)!.item === event.target);
  const next = from < 0 ? undefined : ids[from + (event.key === "ArrowUp" ? -1 : 1)];
  if (!next) return;
  event.preventDefault();
  show(next);
  pages.get(next)!.item.focus();
});

/** A card on a page, under a heading when it has a title. */
function block(parent: HTMLElement, title?: string, className = "card"): HTMLElement {
  const card = el("div", className);
  if (title) {
    const group = el("div", "group");
    group.append(el("h2", undefined, title), card);
    parent.append(group);
  } else {
    parent.append(card);
  }
  return card;
}

const overviewPage = page("overview", "Overview");
// A new install opens Settings here (#152): what to connect first.
const starting = new URLSearchParams(window.location.search).has("start");
const startBlock = block(overviewPage, "Get started");
startBlock.parentElement!.hidden = true;
const overviewBlock = block(overviewPage, undefined, "card overview");
const agentsPage = page("agents", "Agents");
const hooksBlock = block(agentsPage, "Claude Code");
const createBlock = block(agentsPage, undefined, "card card-soon");
const rulesBlock = block(agentsPage, "Always rules");
const islandBlock = block(page("island", "Island"));
const noticesPage = page("notices", "Notices & sounds");
const colorsBlock = block(noticesPage, "Colors");
const soundsBlock = block(noticesPage, "Sounds");
const githubBlock = block(page("github", "GitHub"));
nav.append(el("div", "nav-separator"));
const generalPage = page("general", "General");
const startupBlock = block(generalPage, "Open at login");
const updatesBlock = block(generalPage, "Updates");
const aboutBlock = block(page("about", "About"));
show("overview");

const POSITIONS: Record<IslandPosition, string> = {
  top: "Top center",
  left: "Left side",
  right: "Right side",
  "bottom-left": "Bottom left",
  "bottom-right": "Bottom right",
};
let status: HooksStatus | null = null;
let rules: AlwaysRule[] = [];
let rulesLoad: "pending" | "done" | "failed" = "pending";
let busy = false;
let github: GithubStatus | null = null;
let githubBusy = false;
let githubRevision = 0;
let githubError: string | null = null;
let updates: UpdateStatus | null = null;
let updatesBusy = false;
let updatesError: string | null = null;
let island: IslandPrefs | null = null;
let toggleBusy = false;
let startup: AutostartStatus | null = null;
let startupError: string | null = null;
let startupBusy = false;
let islandError: string | null = null;
let soundsError: string | null = null;
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

function hooksLabel({ installed, outdated }: HooksStatus): string {
  return outdated ? "Connected · update available" : installed ? "Connected" : "Not connected";
}

function renderHooks(): void {
  hooksBlock.replaceChildren();
  if (status) {
    const row = el("div", "status-row");
    row.append(
      el("span", status.installed ? "status-dot connected" : "status-dot"),
      el("span", undefined, hooksLabel(status)),
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
  renderOverview();
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
  renderOverview();
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

function githubLabel({ accounts, state, message }: GithubStatus): string {
  return {
    off: "Off",
    checking: accounts.length > 0 ? "On" : "Checking…",
    ok: "On",
    no_gh: "GitHub CLI not found",
    signed_out: "Signed out",
    error: message ?? "GitHub request failed",
  }[state];
}

function renderGithub(): void {
  renderOverview();
  githubBlock.replaceChildren(
    el("div", "note", "Shows your open pull requests and the reviews waiting for you, using the GitHub CLI (gh) you are already signed in to. Cortex never stores a GitHub token. Checks about once a minute while on."),
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
    const row = el("div", "status-row");
    row.append(el("span", state === "ok" ? "status-dot connected" : "status-dot"), el("span", undefined, githubLabel(github)));
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

function updatesLabel({ state, version, message }: UpdateStatus): string {
  return {
    unsupported: "Not available for this package: update it with your package manager.",
    off: "Off",
    idle: "On · up to date",
    checking: "Checking…",
    downloading: `Downloading Cortex ${version ?? ""}…`,
    ready: `Cortex ${version ?? ""} is ready`,
    error: message ?? "Update check failed",
  }[state];
}

function renderUpdates(): void {
  renderOverview();
  updatesBlock.replaceChildren(
    el("div", "note", "When on, Cortex asks GitHub for a new release at launch and every 6 hours, and sends nothing else. An update is installed only if it carries Cortex's signature, and only when you click Update and restart."),
  );
  if (updates) {
    const { enabled, state } = updates;
    const row = el("div", "status-row");
    row.append(el("span", state === "idle" || state === "ready" ? "status-dot connected" : "status-dot"), el("span", undefined, updatesLabel(updates)));
    updatesBlock.append(row);
    if (state !== "unsupported") {
      const actions = el("div", "actions");
      if (state === "ready") actions.append(button("btn-allow", "Update and restart", () => void changeUpdates(api.updatesStart)));
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
  renderOverview();
  islandBlock.replaceChildren(el("div", "note", "The mascot sits in the island and takes the color of what needs you (see Notices & sounds)."));
  soundsBlock.replaceChildren();
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
    const lively = prefs.lively;
    const liveRow = el("div", "status-row");
    const liveToggle = button(lively ? "btn-neutral" : "btn-allow", lively ? "Turn off" : "Turn on", () => void changeToggle(() => api.islandSetLively(!lively)));
    liveToggle.disabled = toggleBusy;
    liveRow.append(el("span", lively ? "status-dot connected" : "status-dot"), el("span", undefined, lively ? "Lively mascot on" : "Lively mascot off"), liveToggle);
    const liveNote = el("div", "note", "The mascot follows your mouse, blinks, naps after 10 minutes without agents or mouse, and reacts to a click (three quick ones make it dizzy). With Reduce motion on in your system, it does not jump or shake.");
    islandBlock.append(gallery, el("h3", undefined, "Lively mascot"), liveNote, liveRow, el("h3", undefined, "Position"), positions, el("h3", undefined, "Screen"), displays);
    const on = prefs.sound;
    const row = el("div", "status-row");
    const toggle = button(on ? "btn-neutral" : "btn-allow", on ? "Turn off" : "Turn on", () => void changeToggle(() => api.islandSetSound(!on), true));
    toggle.disabled = toggleBusy;
    row.append(el("span", on ? "status-dot connected" : "status-dot"), el("span", undefined, on ? "Sounds on" : "Sounds off"), toggle);
    soundsBlock.append(row);
  }
  if (islandError) islandBlock.append(el("div", "note note-error", islandError));
  if (screensError) islandBlock.append(el("div", "note note-error", screensError));
  // Without the prefs there is no toggle: say why on this page too.
  const soundsMessage = soundsError ?? (prefs ? null : islandError);
  if (soundsMessage) soundsBlock.append(el("div", "note note-error", soundsMessage));
}

// The four colors, each with its sound to try (#131).
const TONES = [
  ["green", "Done", "An agent is done."],
  ["blue", "Review", "Someone asks for your review or approves your pull request."],
  ["amber", "Needs you", "An agent waits for you."],
  ["red", "Failed", "Something failed: an agent, a pull request or an action."],
] as const;
colorsBlock.append(el("div", "note", "The mascot and the island's border take the color of what needs you."));
for (const [tone, name, text] of TONES) {
  const row = el("div", "rule-row");
  const label = el("div", "rule-scope");
  label.append(el("div", undefined, name), el("div", "note", text));
  const play = button("btn-neutral", "Play", () => playSound(tone, true));
  play.setAttribute("aria-label", `Play the ${name} sound`);
  row.append(el("span", `status-dot tone-${tone}`), label, play);
  colorsBlock.append(row);
}

// Agent recipes come later (#92); the maintainer wants the place seen now (#151).
const createLabel = el("div", "rule-scope");
createLabel.append(el("div", "card-title", "Create an agent"), el("div", "note", "Pick the model, the folder, the instructions, the tools and the knowledge sources. Coming in a new version."));
const createButton = button("btn-allow", "Create…", () => undefined);
createButton.disabled = true;
const createRow = el("div", "rule-row");
createRow.append(createLabel, createButton);
createBlock.append(createRow);

// One line per subject, each opening its page. Built once and updated in
// place: rebuilding would drop the keyboard focus on every status event.
const overviewRows = ([
  ["agents", "Claude Code"], ["agents", "Always rules"], ["github", "GitHub"], ["island", "Island"],
  ["notices", "Sounds"], ["general", "Updates"], ["general", "Open at login"],
] as const).map(([id, name]) => {
  const row = el("button", "overview-row");
  const dot = el("span", "status-dot blank");
  const value = el("span", "overview-value", "…");
  const chevron = el("span", "overview-chevron", "›");
  chevron.setAttribute("aria-hidden", "true");
  row.append(dot, el("span", "overview-name", name), value, chevron);
  row.addEventListener("click", () => {
    show(id);
    pages.get(id)!.item.focus();
  });
  overviewBlock.append(row);
  return { dot, value };
});

function startRow(title: string, text: string, label: string, act: () => void): HTMLElement {
  const row = el("div", "rule-row");
  const about = el("div", "rule-scope");
  about.append(el("div", "card-title", title), el("div", "note", text));
  row.append(about, button("btn-allow", label, act));
  startBlock.append(row);
  return row;
}
const startClaude = startRow("Connect Claude Code", "Its sessions show in the island, and you answer its requests from there.", "Connect…", () => {
  show("agents");
  pages.get("agents")!.item.focus();
  void previewHooks(false);
});
const startGithub = startRow("Connect GitHub", "Optional: your pull requests and the reviews waiting for you.", "Set up…", () => {
  show("github");
  pages.get("github")!.item.focus();
  if (github && !github.enabled) void changeGithub(() => api.githubSetEnabled(true));
});

/** "…" while a status loads; a failed load is told here too, not only on its page. */
function renderOverview(): void {
  // Each step shows once its status is known, until it is done.
  startClaude.hidden = !status || status.installed;
  startGithub.hidden = !github || github.state === "ok";
  startBlock.parentElement!.hidden = !starting || (startClaude.hidden && startGithub.hidden);
  const pending = (failed: unknown): [string, null] => [failed ? "Couldn't load" : "…", null];
  const count = rules.length === 0 ? "No rules yet" : `${rules.length} ${rules.length === 1 ? "rule" : "rules"}`;
  const lines: [string, boolean | null][] = [
    status ? [hooksLabel(status), status.installed] : pending(flow.phase === "error"),
    rulesLoad === "done" ? [count, null] : pending(rulesLoad === "failed"),
    github ? [githubLabel(github) + (github.state === "ok" && github.accounts.length > 1 ? ` · ${github.accounts.length} accounts` : ""), github.state === "ok"] : pending(githubError),
    island ? [`${MASCOT_NAMES[island.mascot]} · ${POSITIONS[island.position]}${island.lively ? " · lively" : ""}`, null] : pending(islandError),
    island ? [island.sound ? "On" : "Off", island.sound] : pending(islandError),
    updates ? [updatesLabel(updates), updates.state === "idle" || updates.state === "ready"] : pending(updatesError),
    startup ? [startupLabel(startup), startup.enabled] : pending(startupError),
  ];
  lines.forEach(([text, ok], i) => {
    const { dot, value } = overviewRows[i]!; // Same order as the rows above.
    value.textContent = text;
    dot.className = ok === null ? "status-dot blank" : ok ? "status-dot connected" : "status-dot";
  });
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

async function changeToggle(set: () => Promise<void>, sounds = false): Promise<void> {
  if (toggleBusy) return;
  toggleBusy = true;
  if (sounds) soundsError = null;
  else islandError = null;
  renderIsland();
  try {
    await set();
  } catch (error) {
    if (sounds) soundsError = String(error);
    else islandError = String(error);
  } finally {
    toggleBusy = false;
    renderIsland();
  }
}

function startupLabel({ available, enabled }: AutostartStatus): string {
  return !available ? "Not available for this copy: open Cortex from where it is installed." : enabled ? "On · opens at login" : "Off";
}

function renderStartup(): void {
  renderOverview();
  startupBlock.replaceChildren(el("div", "note", "Cortex opens when you log in, straight into the island, with no opening screen."));
  if (startup) {
    const row = el("div", "status-row");
    row.append(el("span", startup.enabled ? "status-dot connected" : "status-dot"), el("span", undefined, startupLabel(startup)));
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
    rulesLoad = "done";
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
    api.rulesList().then((value) => { if (!rulesChanged) rules = value; rulesLoad = "done"; renderRules(); }).catch((error: unknown) => {
      if (!rulesChanged) rulesLoad = "failed";
      rulesBlock.append(el("div", "note note-error", String(error)));
      renderOverview();
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
