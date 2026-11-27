import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

import { button, el } from "../shared/dom";
import { diffView } from "../shared/diff-view";
import {
  api, HOOKS_EVENT, RULES_EVENT, GITHUB_EVENT, GITHUB_CLI_URL, GITHUB_DEVICE_URL, githubShown,
  type GithubStatus, UPDATES_EVENT, type UpdateStatus, ISLAND_PREFS_EVENT, type IslandPrefs, type AutostartStatus,
  ISLAND_SCREENS_EVENT, type IslandPosition, type IslandScreen,
  type AlwaysRule, type HooksPreview, type HooksStatus,
} from "../shared/ipc";
import { fromRust, type Language, setLanguage, t } from "../shared/i18n";
import { mascotName, MASCOTS, mascotSvg } from "../shared/mascots";
import { alwaysScope } from "../shared/rules";
import { playSound } from "../shared/sounds";

// The language first: every text below is drawn in it (#154).
const shownLanguage: Language = (await api.islandPrefs().catch(() => null))?.language ?? "en";
setLanguage(shownLanguage);
document.title = t("nav.title");

// A portal (#151): one page per subject, picked in the sidebar.
const nav = document.getElementById("settings-nav")!;
nav.setAttribute("aria-label", t("nav.label"));
const root = document.getElementById("settings")!;
type PageId = "overview" | "agents" | "island" | "notices" | "github" | "general" | "about";
const pages = new Map<PageId, { page: HTMLElement; item: HTMLButtonElement }>();
let shownPage: PageId = "overview";

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
  shownPage = id;
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

const overviewPage = page("overview", t("nav.overview"));
// A new install opens Settings here (#152): what to connect first.
const starting = new URLSearchParams(window.location.search).has("start");
const startBlock = block(overviewPage, t("block.start"));
startBlock.parentElement!.hidden = true;
const overviewBlock = block(overviewPage, undefined, "card overview");
const agentsPage = page("agents", t("nav.agents"));
const hooksBlock = block(agentsPage, t("block.claude"));
const createBlock = block(agentsPage, undefined, "card card-soon");
const rulesBlock = block(agentsPage, t("block.rules"));
const islandBlock = block(page("island", t("nav.island")));
const noticesPage = page("notices", t("nav.notices"));
const colorsBlock = block(noticesPage, t("block.colors"));
const soundsBlock = block(noticesPage, t("block.sounds"));
const githubBlock = block(page("github", t("nav.github")));
nav.append(el("div", "nav-separator"));
const generalPage = page("general", t("nav.general"));
const startupBlock = block(generalPage, t("block.startup"));
const updatesBlock = block(generalPage, t("block.updates"));
const languageBlock = block(generalPage, t("block.language"));
const aboutBlock = block(page("about", t("nav.about")));
// Opened again in another language: back on the page it was on.
const returning = window.location.hash.slice(1);
show(pages.has(returning as PageId) ? (returning as PageId) : "overview");

const positionName = (position: IslandPosition): string => t(`position.${position}`);
const POSITIONS: IslandPosition[] = ["top", "left", "right", "bottom-left", "bottom-right"];
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
  return outdated ? t("hooks.outdated") : installed ? t("hooks.connected") : t("hooks.notConnected");
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
      if (status.outdated) actions.append(button("btn-allow", t("hooks.updateButton"), () => void previewHooks(false)));
      actions.append(button("btn-neutral", installed ? t("hooks.removeButton") : t("hooks.installButton"), () => void previewHooks(installed)));
    }
  } else if (flow.phase === "review") {
    const { preview, removing } = flow;
    if (preview.changes) {
      hooksBlock.append(
        el("div", "note", removing ? t("hooks.willRemove") : t("hooks.willAdd")),
        diffView(preview.diff),
      );
      actions.append(
        button("btn-neutral", t("common.cancel"), closeFlow),
        button(removing ? "btn-deny" : "btn-allow", removing ? t("hooks.remove") : t("hooks.install"), () => void confirmHooks(preview, removing)),
      );
    } else {
      hooksBlock.append(el("div", "note", t("hooks.nothing")));
      actions.append(button("btn-neutral", t("common.close"), closeFlow));
    }
  } else {
    if (flow.phase === "error") {
      hooksBlock.append(el("div", "note note-error", fromRust(flow.message)));
    } else {
      hooksBlock.append(el("div", "note", t("hooks.done")));
      if (flow.backup) hooksBlock.append(el("div", "note note-dim", t("hooks.backup", flow.backup)));
    }
    actions.append(button("btn-neutral", t("common.close"), closeFlow));
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
  rulesBlock.replaceChildren(el("div", "note", t("rules.note")));
  if (rulesError) rulesBlock.append(el("div", "note note-error", fromRust(rulesError)));
  if (rules.length === 0) {
    rulesBlock.append(el("div", "note empty", t("rules.none")));
    return;
  }
  for (const rule of rules) {
    const row = el("div", "rule-row");
    row.append(el("span", "rule-scope", alwaysScope(rule)), button("btn-neutral", t("rules.remove"), () => void changeRules(() => api.ruleRemove(rule))));
    rulesBlock.append(row);
  }
  const actions = el("div", "actions");
  const removeAll = button("btn-deny", t("rules.removeAll"), () => {
    if (confirmationTimer !== undefined) {
      clearConfirmation();
      renderRules();
      void changeRules(() => api.rulesClear());
      return;
    }
    removeAll.textContent = t("rules.confirm", rules.length);
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
    off: t("common.off"),
    checking: accounts.length > 0 ? t("common.on") : t("common.checking"),
    ok: t("common.on"),
    no_gh: t("github.noGh"),
    signed_out: t("github.signedOut"),
    error: message ? fromRust(message) : t("github.failed"),
  }[state];
}

function renderGithub(): void {
  renderOverview();
  githubBlock.replaceChildren(
    el("div", "note", t("github.note")),
    el("div", "note", t("github.accountsNote")),
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
        if (account.active) label.append(el("span", "note", t("github.active")));
        if (account.message) label.append(el("div", "note", fromRust(account.message)));
        row.append(
          el("span", account.state === "ok" ? "status-dot connected" : `status-dot github-status-${account.state}`),
          label,
          action("btn-neutral", account.shown ? t("github.hide") : t("github.show"), () => api.githubSetAccountShown(account.login, !account.shown)),
        );
        githubBlock.append(row);
      }
      if (state === "no_gh") {
        if (installer) {
          actions.append(action("btn-allow", t("github.install"), api.githubInstall, running));
        } else {
          actions.append(action("btn-neutral", t("github.get"), () => api.openUrl(GITHUB_CLI_URL)));
          if (/Linux/i.test(navigator.userAgent)) {
            githubBlock.append(el("div", "note", t("github.linuxNote")));
          }
        }
      } else if (accounts.length > 0 || state === "signed_out") {
        actions.append(action("btn-allow", state === "signed_out" ? t("github.signIn") : t("github.addAccount"), api.githubSignIn, running));
      }
      actions.append(action("btn-neutral", t("github.refresh"), api.githubRefresh));
    }
    actions.append(action(enabled ? "btn-neutral" : "btn-allow", enabled ? t("common.turnOff") : t("common.turnOn"), () => api.githubSetEnabled(!enabled)));
    githubBlock.append(actions);

    if (task.kind === "installing" || task.kind === "signing_in") {
      const area = el("div", "status-row github-task");
      if (task.kind === "installing") {
        area.append(el("span", undefined, t("github.installing")));
      } else if (task.code) {
        area.append(
          el("span", undefined, t("github.enterCode")),
          el("code", "github-device-code", task.code),
          action("btn-allow", t("github.open"), () => api.openUrl(GITHUB_DEVICE_URL)),
        );
      } else {
        area.append(
          el("span", undefined, t("github.starting")),
          el("span", "note", t("github.noCode")),
        );
      }
      area.append(action("btn-neutral", t("common.cancel"), api.githubCancelTask));
      githubBlock.append(area);
    } else if (task.kind === "failed") {
      githubBlock.append(el("div", "note note-error", fromRust(task.message)));
    }
  }
  if (githubError) githubBlock.append(el("div", "note note-error", fromRust(githubError)));
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
    unsupported: t("updates.unsupported"),
    off: t("common.off"),
    idle: t("updates.idle"),
    checking: t("common.checking"),
    downloading: t("updates.downloading", version ?? ""),
    ready: t("updates.ready", version ?? ""),
    error: message ? fromRust(message) : t("updates.failed"),
  }[state];
}

function renderUpdates(): void {
  renderOverview();
  updatesBlock.replaceChildren(
    el("div", "note", t("updates.note")),
  );
  if (updates) {
    const { enabled, state } = updates;
    const row = el("div", "status-row");
    row.append(el("span", state === "idle" || state === "ready" ? "status-dot connected" : "status-dot"), el("span", undefined, updatesLabel(updates)));
    updatesBlock.append(row);
    if (state !== "unsupported") {
      const actions = el("div", "actions");
      if (state === "ready") actions.append(button("btn-allow", t("updates.restart"), () => void changeUpdates(api.updatesStart)));
      const on = enabled === true;
      actions.append(button(on ? "btn-neutral" : "btn-allow", on ? t("common.turnOff") : t("common.turnOn"), () => void changeUpdates(() => api.updatesSetEnabled(!on))));
      actions.querySelectorAll("button").forEach((node) => { node.disabled = updatesBusy; });
      updatesBlock.append(actions);
    }
  }
  if (updatesError) updatesBlock.append(el("div", "note note-error", fromRust(updatesError)));
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
  islandBlock.replaceChildren(el("div", "note", t("island.note")));
  soundsBlock.replaceChildren();
  const prefs = island;
  if (prefs) {
    const gallery = el("div", "mascot-gallery");
    for (const mascot of MASCOTS) {
      const choice = islandChoice("", mascot === prefs.mascot, () => api.islandSetMascot(mascot));
      choice.title = mascotName(mascot);
      choice.append(mascotSvg(mascot) ?? el("span", "mascot-none"), el("span", "mascot-name", mascotName(mascot)));
      gallery.append(choice);
    }

    const positions = el("div", "mascot-gallery");
    for (const position of POSITIONS) {
      positions.append(islandChoice(positionName(position), position === prefs.position, () => api.islandSetPosition(position)));
    }

    const displays = el("div", "mascot-gallery screen-gallery");
    displays.append(islandChoice(t("island.mainScreen"), prefs.screen === null, () => api.islandSetScreen(null)));
    for (const screen of screens ?? []) {
      displays.append(islandChoice(
        `${screen.name} · ${screen.width} × ${screen.height}${screen.primary ? t("island.main") : ""}`,
        screen.name === prefs.screen,
        () => api.islandSetScreen(screen.name),
      ));
    }
    if (screens && prefs.screen !== null && !screens.some(({ name }) => name === prefs.screen)) {
      displays.append(islandChoice(t("island.notConnected", prefs.screen), true));
    }
    const lively = prefs.lively;
    const liveRow = el("div", "status-row");
    const liveToggle = button(lively ? "btn-neutral" : "btn-allow", lively ? t("common.turnOff") : t("common.turnOn"), () => void changeToggle(() => api.islandSetLively(!lively)));
    liveToggle.disabled = toggleBusy;
    liveRow.append(el("span", lively ? "status-dot connected" : "status-dot"), el("span", undefined, lively ? t("island.livelyOn") : t("island.livelyOff")), liveToggle);
    const liveNote = el("div", "note", t("island.livelyNote"));
    const along: HTMLElement[] = [];
    if (prefs.drags) {
      const center = el("div", "status-row");
      center.append(el("span", "note", t("island.dragNote")), islandChoice(t("island.center"), false, prefs.offset === null ? undefined : () => api.islandCenter()));
      along.push(center);
    }
    islandBlock.append(gallery, el("h3", undefined, t("island.lively")), liveNote, liveRow, el("h3", undefined, t("island.position")), positions, ...along, el("h3", undefined, t("island.screen")), displays);
    const on = prefs.sound;
    const row = el("div", "status-row");
    const toggle = button(on ? "btn-neutral" : "btn-allow", on ? t("common.turnOff") : t("common.turnOn"), () => void changeToggle(() => api.islandSetSound(!on), true));
    toggle.disabled = toggleBusy;
    row.append(el("span", on ? "status-dot connected" : "status-dot"), el("span", undefined, on ? t("island.soundsOn") : t("island.soundsOff")), toggle);
    soundsBlock.append(row);
  }
  if (islandError) islandBlock.append(el("div", "note note-error", fromRust(islandError)));
  if (screensError) islandBlock.append(el("div", "note note-error", fromRust(screensError)));
  // Without the prefs there is no toggle: say why on this page too.
  const soundsMessage = soundsError ?? (prefs ? null : islandError);
  if (soundsMessage) soundsBlock.append(el("div", "note note-error", fromRust(soundsMessage)));
}

// The colors, each with its sound to try (#131): Atlantis's turquoise too (#173).
const TONES = ["green", "atlantis", "blue", "amber", "red", "pink"] as const;
colorsBlock.append(el("div", "note", t("colors.note")));
for (const tone of TONES) {
  const name = t(`tone.${tone}`);
  const row = el("div", "rule-row");
  const label = el("div", "rule-scope");
  label.append(el("div", undefined, name), el("div", "note", t(`tone.${tone}.text`)));
  const play = button("btn-neutral", t("colors.play"), () => playSound(tone, true));
  play.setAttribute("aria-label", t("colors.playLabel", name));
  row.append(el("span", `status-dot tone-${tone}`), label, play);
  colorsBlock.append(row);
}

// Agent recipes come later (#92); the maintainer wants the place seen now (#151).
const createLabel = el("div", "rule-scope");
createLabel.append(el("div", "card-title", t("create.title")), el("div", "note", t("create.note")));
const createButton = button("btn-allow", t("create.button"), () => undefined);
createButton.disabled = true;
const createRow = el("div", "rule-row");
createRow.append(createLabel, createButton);
createBlock.append(createRow);

// One line per subject, each opening its page. Built once and updated in
// place: rebuilding would drop the keyboard focus on every status event.
const overviewRows = ([
  ["agents", t("block.claude")], ["agents", t("block.rules")], ["github", t("nav.github")], ["island", t("nav.island")],
  ["notices", t("block.sounds")], ["general", t("block.updates")], ["general", t("block.startup")],
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
const startClaude = startRow(t("start.claude"), t("start.claudeNote"), t("start.claudeButton"), () => {
  show("agents");
  pages.get("agents")!.item.focus();
  void previewHooks(false);
});
const startGithub = startRow(t("start.github"), t("start.githubNote"), t("start.githubButton"), () => {
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
  const pending = (failed: unknown): [string, null] => [failed ? t("common.couldntLoad") : "…", null];
  const count = rules.length === 0 ? t("rules.noneShort") : t("rules.count", rules.length);
  const lines: [string, boolean | null][] = [
    status ? [hooksLabel(status), status.installed] : pending(flow.phase === "error"),
    rulesLoad === "done" ? [count, null] : pending(rulesLoad === "failed"),
    github ? [githubLabel(github) + (github.state === "ok" && github.accounts.length > 1 ? t("github.accounts", github.accounts.length) : ""), github.state === "ok"] : pending(githubError),
    island ? [`${mascotName(island.mascot)} · ${positionName(island.position)}${island.lively ? t("island.livelyShort") : ""}`, null] : pending(islandError),
    island ? [island.sound ? t("common.on") : t("common.off"), island.sound] : pending(islandError),
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

let languageError: string | null = null;
let languageBusy = false;

/** English, Portuguese, or the system's; every window follows at once. */
function renderLanguage(): void {
  languageBlock.replaceChildren(el("div", "note", t("language.note")));
  const choices = el("div", "mascot-gallery");
  const picked = island?.language_choice ?? null;
  for (const [choice, label] of [[null, t("language.system")], ["en", "English"], ["pt", "Português"]] as const) {
    const node = button(choice === picked ? "mascot-choice selected" : "mascot-choice", label, () => void setLanguageChoice(choice));
    node.setAttribute("aria-pressed", String(choice === picked));
    node.disabled = languageBusy || !island;
    if (choice) node.lang = choice === "pt" ? "pt-BR" : "en";
    choices.append(node);
  }
  languageBlock.append(choices);
  if (languageError) languageBlock.append(el("div", "note note-error", fromRust(languageError)));
}

async function setLanguageChoice(choice: Language | null): Promise<void> {
  if (languageBusy) return;
  languageBusy = true;
  languageError = null;
  renderLanguage();
  try {
    await api.languageSet(choice);
  } catch (error) {
    languageError = String(error);
  } finally {
    languageBusy = false;
    renderLanguage();
  }
}

function startupLabel({ available, enabled }: AutostartStatus): string {
  return !available ? t("startup.unavailable") : enabled ? t("startup.on") : t("common.off");
}

function renderStartup(): void {
  renderOverview();
  startupBlock.replaceChildren(el("div", "note", t("startup.note")));
  if (startup) {
    const row = el("div", "status-row");
    row.append(el("span", startup.enabled ? "status-dot connected" : "status-dot"), el("span", undefined, startupLabel(startup)));
    startupBlock.append(row);
    if (startup.available || startup.enabled) {
      const on = startup.enabled;
      const toggle = button(on ? "btn-neutral" : "btn-allow", on ? t("common.turnOff") : t("common.turnOn"), () => void setStartup(!on));
      toggle.disabled = startupBusy;
      startupBlock.append(toggle);
    }
  }
  if (startupError) startupBlock.append(el("div", "note note-error", fromRust(startupError)));
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
    // A rebuild takes the keyboard focus off its buttons: only for a change.
    const same = githubShown(github) === githubShown(event.payload);
    github = event.payload;
    if (!same) renderGithub();
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
    // Another language: drawn again from the top, on the page it was on.
    if (event.payload.language !== shownLanguage) {
      window.location.hash = shownPage;
      window.location.reload();
      return;
    }
    renderIsland();
    renderLanguage();
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
    api.islandPrefs().then((value) => { if (!islandChanged) island = value; }).catch((error: unknown) => { islandError = String(error); }).finally(() => { renderIsland(); renderLanguage(); }),
    api.updatesStatus().then((value) => { if (!updatesChanged) updates = value; }).catch((error: unknown) => { updatesError = String(error); }).finally(renderUpdates),
    api.githubStatus().then((value) => { if (!githubChanged) github = value; }).catch((error: unknown) => { githubError = String(error); }).finally(renderGithub),
    api.hooksStatus().then((value) => { if (!hooksChanged) status = value; }).catch((error: unknown) => {
      flow = { phase: "error", message: String(error) };
    }).finally(renderHooks),
    api.rulesList().then((value) => { if (!rulesChanged) rules = value; rulesLoad = "done"; renderRules(); }).catch((error: unknown) => {
      if (!rulesChanged) rulesLoad = "failed";
      rulesBlock.append(el("div", "note note-error", fromRust(String(error))));
      renderOverview();
    }),
    getVersion().then((version) => {
      const commit = import.meta.env.VITE_CORTEX_COMMIT;
      aboutBlock.prepend(el("div", undefined, commit ? t("about.test", version, commit) : t("about.version", version)));
      // What's new opens in the app (#180); its "See it on GitHub" has the notes.
      aboutActions.prepend(button("btn-neutral", t("about.news"), () => {
        api.newsOpen().catch((error: unknown) => { aboutBlock.append(el("div", "note note-error", fromRust(String(error)))); });
      }));
    }).catch((error: unknown) => {
      aboutBlock.prepend(el("div", "note note-error", fromRust(String(error))));
    }),
  ]);
}

const aboutActions = el("div", "actions");
aboutActions.append(button("btn-neutral", t("about.quit"), () => void api.quit()));
aboutBlock.append(el("div", "note", t("about.note")), aboutActions);
window.addEventListener("pagehide", clearConfirmation);
void start();
