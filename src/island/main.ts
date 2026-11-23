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
  ISLAND_PREFS_EVENT,
  type IslandPrefs,
  OUTSIDE_CLICK_EVENT,
  type PendingApproval,
  type SessionView,
  type Snapshot,
  STATE_EVENT,
  UPDATES_EVENT,
  type UpdateStatus,
} from "../shared/ipc";
import { type Mascot, mascotSvg } from "../shared/mascots";
import { playSound } from "../shared/sounds";
import { approvalCard, pruneSelections, questionCard, sessionCard, stateLabel } from "./cards";
import { githubCard, shownAccounts } from "./github";
import { agentShort, agentTag, byUrgency, prIcon, sessionKey, sessionTone, type Tone, topTone } from "./notch";
import { type Onboarding, onboardingCard } from "./onboarding";
import { askingUpdates, updateChip, updatesQuestion } from "./updates";

const MAX_HEIGHT = 440;
/** Agent tags shown side by side; the rest becomes a counter. */
const MAX_TAGS = 4;
/** How long the pointer must be gone before an idle island folds. */
const HIDE_DELAY_MS = 700;

const root = document.getElementById("island")!;

let snapshot: Snapshot = { sessions: [], approvals: [] };
let expanded = false;
let github: GithubStatus | null = null;
let updates: UpdateStatus | null = null;
let updatesError: string | null = null;
let mascot: Mascot = "eyes";
let sound = true;
let soundReady = false;
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
/** What the folded island shows, or `null` while unfolded. */
let folded: string | null = null;
/** Each session's last event when the user last looked at the island. */
const seen = new Map<string, number>();
/** Each session's color from its state alone (not what was seen), to ring on a real change. */
const previousTones = new Map<string, Tone>();
/** Cards already rung for: a new card rings even when its session was amber already. */
const rungCards = new Set<string>();
/** Green (finished) or red (failed) sessions nobody looked at yet, kept after the session is gone. */
const unseen = new Map<string, Tone>();
let tone: Tone = "gray";
/** The color just rose: the next mascot drawn jumps, once. */
let bounce = false;

/**
 * No card demanding eyes and nobody looking: the island folds into the
 * notch, even with live sessions (hover shows them) or with cards the user
 * folded away. It stays up while the hooks are missing, so "Connect Claude
 * Code" is seen.
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

const toneOf = (session: SessionView): Tone =>
  sessionTone(session, (seen.get(sessionKey(session)) ?? -1) >= session.last_event_ms);

/** The island's color: the most urgent of the live sessions and the unseen ones. */
function updateTone(): void {
  // What changes while the user is looking has been seen.
  if (hovered) acknowledge();
  const live = new Set<string>();
  let notice: Tone = "gray";
  for (const session of snapshot.sessions) {
    const key = sessionKey(session);
    live.add(key);
    const t = toneOf(session);
    const state = sessionTone(session, false);
    if (state !== previousTones.get(key)) notice = topTone([notice, state]);
    previousTones.set(key, state);
    if (t === "green" || t === "red") unseen.set(key, t);
    else unseen.delete(key);
  }
  for (const key of seen.keys()) if (!live.has(key)) seen.delete(key);
  for (const key of previousTones.keys()) if (!live.has(key)) previousTones.delete(key);
  const cards = new Set(snapshot.approvals.map((a) => a.request_id));
  for (const id of cards) if (!rungCards.has(id)) notice = topTone([notice, "amber"]);
  for (const id of rungCards) if (!cards.has(id)) rungCards.delete(id);
  for (const id of cards) rungCards.add(id);
  if (soundReady && sound && notice !== "gray") playSound(notice);
  const next = topTone([...snapshot.sessions.map(toneOf), ...unseen.values()]);
  if (topTone([next, tone]) !== tone && next !== tone) bounce = true;
  tone = next;
}

/** The user looked: finished and failed sessions go back to gray. */
function acknowledge(): void {
  for (const session of snapshot.sessions) seen.set(sessionKey(session), session.last_event_ms);
  unseen.clear();
}

/** Pull requests waiting for your review, and your own with failing checks. */
function prTotals(): { reviews: number; failing: number } {
  let reviews = 0;
  let failing = 0;
  if (github?.enabled) {
    for (const { summary } of shownAccounts(github.accounts)) {
      reviews += summary.review_requests_total;
      failing += summary.mine.filter((pr) => pr.checks === "failing").length;
    }
  }
  return { reviews, failing };
}

/** Everything the folded notch shows: the same key means nothing to repaint. */
function foldedKey(): string {
  const { reviews, failing } = prTotals();
  return `${tone}|${mascot}|${reviews}|${failing}|${bounce}`;
}

/** Touch the class only when it changes: any DOM change costs a repaint (#51). */
function setClass(open: boolean): void {
  const name = `island tone-${tone}${open ? " island--open" : ""}`;
  if (root.className !== name) root.className = name;
}

// ---------------------------------------------------------------------------
// Rendering

function render(): void {
  const pending = new Set(snapshot.approvals.map((a) => a.request_id));
  dismissed = new Set([...dismissed].filter((id) => pending.has(id)));
  pruneSelections(pending);
  updateTone();

  if (isHidden()) {
    expanded = false;
    autoExpanded = false;
    // Same notch: paint nothing. Each paint with the screen locked keeps WebKit
    // retrying to free its buffers, which costs ~1% CPU until unlock (#51).
    const key = foldedKey();
    if (folded === key) return;
    setClass(false);
    const node = el("div", "notch");
    node.addEventListener("click", () => setHovered(true));
    node.append(mascotNode());
    const totals = prTotals();
    if (totals.reviews > 0 || totals.failing > 0) node.append(prCount(totals));
    folded = foldedKey();
    root.replaceChildren(node);
    void fitWindowToContent();
    return;
  }
  folded = null;

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

  const panel = panelContent();
  setClass(panel.length > 0);
  root.replaceChildren(bar());
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

/**
 * The mascot in the island's color, or a dot when the user hid it. The jump
 * is spent on the first mascot drawn after the color rose: a redraw never
 * starts it again, so it cannot loop.
 */
function mascotNode(): Element {
  const node: Element = mascotSvg(mascot) ?? el("span", "brand-dot");
  if (bounce) {
    bounce = false;
    node.classList.add("bounce");
  }
  return node;
}

function prCount({ reviews, failing }: { reviews: number; failing: number }): HTMLElement {
  const node = el("span", "pr-count");
  const notes = [`${reviews} pull request${reviews === 1 ? "" : "s"} waiting for your review`];
  node.append(prIcon());
  if (reviews > 0 || failing === 0) node.append(el("span", undefined, String(reviews)));
  if (failing > 0) {
    node.append(el("span", "pr-failing"));
    notes.push(`${failing} of yours failing checks`);
  }
  node.title = notes.join(" · ");
  return node;
}

/** A click outside folds the island, cards included; the notch stays amber while they wait. */
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
      acknowledge();
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

function toggle(): void {
  expanded = !expanded;
  autoExpanded = false;
  render();
}

/** Agent tags around the mascot, the most urgent first; the PR count; chips and the gear. */
function bar(): HTMLElement {
  const node = el("div", "bar");
  const sessions = byUrgency(snapshot.sessions, toneOf);
  const chips = updateChips();
  // An update chip takes the room of two tags: the bar never outgrows the window.
  const room = chips.length > 0 ? 2 : MAX_TAGS;
  const tags = sessions.slice(0, room).map((session) => agentTag(session, toneOf(session), sessionLabel(session)));
  const center = el("span", "bar-center");
  center.append(mascotNode());
  if (snapshot.approvals.length > 1) center.append(el("span", "queue", `1 of ${snapshot.approvals.length}`));
  node.append(...tags.slice(0, 2), center, ...tags.slice(2));
  if (sessions.length > room) node.append(el("span", "more", `+${sessions.length - room}`));

  const accounts = github?.enabled ? shownAccounts(github.accounts) : [];
  if (accounts.length > 0) {
    const pr = button("btn-pr", "", toggle);
    pr.append(prCount(prTotals()));
    node.append(pr);
  }
  if (hooksInstalled === false && onboarding.phase === "closed" && sessions.length === 0) {
    node.append(button("btn-chip", "Connect Claude Code", () => void openOnboarding()));
  }
  node.append(...chips, settingsButton());
  if (sessions.length > 0) node.addEventListener("click", toggle);
  return node;
}

function sessionLabel(session: SessionView): string {
  return session.current ?? session.ticker.at(-1)?.label ?? stateLabel(session.state);
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

/** One card at a time; the ones after it wait in view below. */
function cardsInView(): HTMLElement[] {
  const [first, ...rest] = snapshot.approvals;
  if (!first) return [];
  const card = first.questions.length > 0 ? questionCard(first, render) : approvalCard(first);
  return rest.length === 0 ? [card] : [card, nextUp(rest)];
}

function nextUp(rest: PendingApproval[]): HTMLElement {
  const node = el("div", "next-up");
  for (const approval of rest.slice(0, 3)) {
    const row = el("div", "next-row");
    row.append(
      el("span", "tag-short", agentShort(approval.session.agent)),
      el("span", "next-text", approval.questions.length > 0 ? "has a question" : `wants ${approval.tool_name} ${approval.detail}`),
    );
    node.append(row);
  }
  if (rest.length > 3) node.append(el("div", "next-row next-more", `+${rest.length - 3} more`));
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

  const accounts = github?.enabled ? shownAccounts(github.accounts) : [];
  return [...question, ...cardsInView(), ...snapshot.sessions.map(sessionCard), ...(accounts.length > 0 ? [githubCard(accounts)] : [])];
}

/** Size the window to what is drawn, so no invisible area eats clicks. */
async function fitWindowToContent(): Promise<void> {
  const rect = root.getBoundingClientRect();
  await api.islandFit(Math.ceil(rect.width), Math.min(MAX_HEIGHT, Math.ceil(rect.height)));
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
  let islandChanged = false;
  await listen<IslandPrefs>(ISLAND_PREFS_EVENT, (event) => {
    islandChanged = true;
    ({ mascot, sound } = event.payload);
    render();
  });
  await listen<boolean>(HOVER_EVENT, (event) => setHovered(event.payload));
  await listen(OUTSIDE_CLICK_EVENT, foldAway);
  // Windows / Linux: the island only learns of an outside click if it had focus.
  window.addEventListener("blur", foldAway);
  document.documentElement.addEventListener("mouseenter", () => setHovered(true));
  document.documentElement.addEventListener("mouseleave", () => setHovered(false));
  let snapshotChanged = false;
  await listen<Snapshot>(STATE_EVENT, (event) => {
    snapshotChanged = true;
    snapshot = event.payload;
    render();
  });
  const [initialSnapshot] = await Promise.all([api.snapshot(), refreshHooksStatus(), api.githubStatus().then((value) => { if (!githubChanged) github = value; }).catch(() => { /* GitHub is optional. */ }), api.updatesStatus().then((value) => { if (!updatesChanged) updates = value; }).catch(() => { /* Without a status the question waits for the next event. */ }), api.islandPrefs().then((value) => { if (!islandChanged) ({ mascot, sound } = value); }).catch(() => { /* Keep the island defaults. */ })]);
  if (!snapshotChanged) snapshot = initialSnapshot;
  render();
  soundReady = true;
}

void start();
