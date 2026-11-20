/**
 * The island. Dumb by design: it draws the latest snapshot from Rust and
 * forwards clicks back. The only state kept here is view state (what is
 * expanded, which options are ticked, where the onboarding flow is).
 */

import { listen } from "@tauri-apps/api/event";

import { button, el } from "../shared/dom";
import {
  api,
  HOOKS_EVENT,
  GITHUB_EVENT,
  type GithubStatus,
  HOVER_EVENT,
  type HooksStatus,
  OUTSIDE_CLICK_EVENT,
  type SessionView,
  type Snapshot,
  STATE_EVENT,
  UPDATES_EVENT,
  type UpdateStatus,
} from "../shared/ipc";
import { approvalCard, pruneSelections, questionCard, sessionCard, stateLabel } from "./cards";
import { githubCard, githubPill, shownAccounts } from "./github";
import { type Onboarding, onboardingCard } from "./onboarding";
import { askingUpdates, updateChip, updatesQuestion } from "./updates";

/** Must match `ISLAND_WIDTH` in Rust and the window width in tauri.conf.json. */
const WINDOW_WIDTH = 460;
const MAX_HEIGHT = 440;
/** The folded island: just a handle to hover. */
const HANDLE_WIDTH = 72;
const HANDLE_HEIGHT = 10;
/** How long the pointer must be gone before an idle island folds. */
const HIDE_DELAY_MS = 700;

const root = document.getElementById("island")!;

let snapshot: Snapshot = { sessions: [], approvals: [] };
let expanded = false;
let github: GithubStatus | null = null;
let updates: UpdateStatus | null = null;
let updatesError: string | null = null;
/** True when the panel was opened by an approval, not by the user. */
let autoExpanded = false;
/** `null` until Rust answers; the "Connect" chip shows only on a definite `false`. */
let hooksInstalled: boolean | null = null;
let onboarding: Onboarding = { phase: "closed" };
/** The pointer is over the island (or left it less than `HIDE_DELAY_MS` ago). */
let hovered = false;
let hideTimer: number | undefined;
/** Cards folded away by a click outside the island; a new card opens it again. */
let dismissed = new Set<string>();

/**
 * No card demanding eyes and nobody looking: the island folds into its handle,
 * even with live sessions (hover shows them) or with cards the user folded
 * away. It stays up while the hooks are missing, so "Connect Claude Code" is seen.
 */
function isHidden(): boolean {
  return (
    snapshot.approvals.every((a) => dismissed.has(a.request_id)) &&
    onboarding.phase === "closed" &&
    hooksInstalled !== false &&
    !askingUpdates(updates) &&
    !hovered
  );
}

// ---------------------------------------------------------------------------
// Rendering

function render(): void {
  const pending = new Set(snapshot.approvals.map((a) => a.request_id));
  dismissed = new Set([...dismissed].filter((id) => pending.has(id)));
  pruneSelections(pending);

  const hidden = isHidden();
  root.classList.toggle("island--hidden", hidden);
  if (hidden) {
    expanded = false;
    autoExpanded = false;
    root.replaceChildren(handle(pending.size > 0));
    void api.islandFit(HANDLE_WIDTH, HANDLE_HEIGHT);
    return;
  }

  const hasCards = snapshot.approvals.length > 0;
  // A card demands eyes: open the panel, and fold it back once the card is
  // gone — unless the user opened it themselves.
  if (hasCards && !expanded) {
    expanded = true;
    autoExpanded = true;
  } else if (!hasCards && autoExpanded) {
    expanded = false;
    autoExpanded = false;
  }

  // A redraw (any agent event) must not scroll the user back to the top.
  const scrolled = new Map<string, number>();
  root.querySelectorAll<HTMLElement>("[data-scroll]").forEach((n) => scrolled.set(n.dataset.scroll ?? "", n.scrollTop));

  root.replaceChildren(bar());
  const panel = panelContent();
  if (panel.length > 0) {
    const node = el("div", "panel");
    node.dataset.scroll = "panel";
    node.append(...panel);
    root.append(node);
  }
  root.querySelectorAll<HTMLElement>("[data-scroll]").forEach((n) => {
    const top = scrolled.get(n.dataset.scroll ?? "");
    if (top) n.scrollTop = top;
  });
  void fitWindowToContent();
}

/** Hovering reveals the island; a click does too, where hover is not reported. */
function handle(waiting: boolean): HTMLElement {
  const node = el("div", waiting ? "handle handle--waiting" : "handle");
  node.addEventListener("click", () => setHovered(true));
  return node;
}

/** A click outside folds the island, cards included; the handle shows they still wait. */
function foldAway(): void {
  if (snapshot.approvals.length === 0) return;
  for (const approval of snapshot.approvals) dismissed.add(approval.request_id);
  window.clearTimeout(hideTimer);
  hideTimer = undefined;
  hovered = false;
  render();
}

function setHovered(inside: boolean): void {
  window.clearTimeout(hideTimer);
  hideTimer = undefined;
  if (inside) {
    if (!hovered) {
      hovered = true;
      render();
    }
    return;
  }
  if (!hovered) return;
  // One short timer, only after the pointer left: brushing past the edge
  // must not fold the island, and a folded island runs no timers at all.
  hideTimer = window.setTimeout(() => {
    hideTimer = undefined;
    hovered = false;
    if (isHidden()) render();
  }, HIDE_DELAY_MS);
}

function bar(): HTMLElement {
  const node = el("div", "bar");
  node.append(el("span", "brand-dot"));
  const accounts = github?.enabled ? shownAccounts(github.accounts) : [];

  if (snapshot.sessions.length === 0 && snapshot.approvals.length === 0) {
    node.append(el("span", "bar-idle", "cortex"));
    if (accounts.length > 0) node.append(githubPill(accounts, () => {
      expanded = !expanded;
      autoExpanded = false;
      render();
    }));
    if (hooksInstalled === false && onboarding.phase === "closed") {
      node.append(button("btn-chip", "Connect Claude Code", () => void openOnboarding()));
    }
    node.append(...updateChips(), settingsButton());
    return node;
  }

  for (const session of snapshot.sessions.slice(0, 3)) {
    node.append(pill(session));
  }
  if (accounts.length > 0) node.append(githubPill(accounts));
  node.append(...updateChips(), settingsButton());
  node.addEventListener("click", () => {
    expanded = !expanded;
    autoExpanded = false;
    render();
  });
  return node;
}

/** Never offered over a card: restarting would drop the agent's waiting request. */
function updateChips(): HTMLElement[] {
  if (snapshot.approvals.length > 0) return [];
  return updateChip(updates, () => void installUpdate(), () => void api.openSettings());
}

function settingsButton(): HTMLButtonElement {
  const node = button("btn-icon", "\u2699\uFE0E", () => void api.openSettings());
  node.title = "Settings";
  return node;
}

function pill(session: SessionView): HTMLElement {
  const node = el("div", "pill");
  node.append(el("span", `state-dot state-${session.state}`));
  node.append(el("span", "agent", session.key.agent));
  const label = session.current ?? session.ticker.at(-1)?.label ?? stateLabel(session.state);
  node.append(el("span", "ticker", label));
  return node;
}

function panelContent(): HTMLElement[] {
  if (onboarding.phase !== "closed") {
    return [
      onboardingCard(onboarding, {
        install: (token) => void installHooks(token),
        close: () => {
          onboarding = { phase: "closed" };
          render();
        },
      }),
    ];
  }
  const question = askingUpdates(updates) ? [updatesQuestion((enabled) => void answerUpdates(enabled), updatesError)] : [];
  if (!expanded) return question;

  const cards = snapshot.approvals.map((approval) =>
    approval.questions.length > 0 ? questionCard(approval, render) : approvalCard(approval),
  );
  const accounts = github?.enabled ? shownAccounts(github.accounts) : [];
  return [...question, ...cards, ...snapshot.sessions.map(sessionCard), ...(accounts.length > 0 ? [githubCard(accounts)] : [])];
}

/** Size the window to what is drawn, so no invisible area eats clicks. */
async function fitWindowToContent(): Promise<void> {
  const height = Math.min(MAX_HEIGHT, Math.ceil(root.getBoundingClientRect().height) + 8);
  await api.islandFit(WINDOW_WIDTH, height);
}

// ---------------------------------------------------------------------------
// Onboarding flow

async function refreshHooksStatus(): Promise<void> {
  try {
    hooksInstalled = (await api.hooksStatus()).installed;
  } catch {
    hooksInstalled = null; // unknown: do not nag
  }
}

async function openOnboarding(): Promise<void> {
  try {
    onboarding = { phase: "review", preview: await api.hooksPreview() };
  } catch (error) {
    onboarding = { phase: "error", message: String(error) };
  }
  render();
}

async function installHooks(token: string): Promise<void> {
  try {
    const result = await api.hooksInstall(token);
    onboarding = { phase: "done", backupPath: result.backup_path };
    hooksInstalled = true;
  } catch (error) {
    onboarding = { phase: "error", message: String(error) };
  }
  render();
}

// ---------------------------------------------------------------------------
// Updates

async function answerUpdates(enabled: boolean): Promise<void> {
  try {
    await api.updatesSetEnabled(enabled);
    updatesError = null;
  } catch (error) {
    updatesError = String(error);
  }
  render();
}

async function installUpdate(): Promise<void> {
  try {
    await api.updatesInstall();
  } catch {
    // Rust reports a failed install through the update status ("Update failed").
  }
}

// ---------------------------------------------------------------------------

async function start(): Promise<void> {
  let githubChanged = false;
  await listen<GithubStatus>(GITHUB_EVENT, (event) => {
    githubChanged = true;
    github = event.payload;
    render();
  });
  let updatesChanged = false;
  await listen<UpdateStatus>(UPDATES_EVENT, (event) => {
    updatesChanged = true;
    updates = event.payload;
    render();
  });
  await listen<HooksStatus>(HOOKS_EVENT, (event) => {
    hooksInstalled = event.payload.installed;
    render();
  });
  await listen<boolean>(HOVER_EVENT, (event) => setHovered(event.payload));
  await listen(OUTSIDE_CLICK_EVENT, foldAway);
  // Windows / Linux: the island only learns of an outside click if it had focus.
  window.addEventListener("blur", foldAway);
  document.documentElement.addEventListener("mouseenter", () => setHovered(true));
  document.documentElement.addEventListener("mouseleave", () => setHovered(false));
  await listen<Snapshot>(STATE_EVENT, (event) => {
    snapshot = event.payload;
    render();
  });
  [snapshot] = await Promise.all([api.snapshot(), refreshHooksStatus(), api.githubStatus().then((value) => { if (!githubChanged) github = value; }).catch(() => { /* GitHub is optional. */ }), api.updatesStatus().then((value) => { if (!updatesChanged) updates = value; }).catch(() => { /* Without a status the question waits for the next event. */ })]);
  render();
}

void start();
