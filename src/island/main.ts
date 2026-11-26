/**
 * The island. Dumb by design: it draws the latest snapshot from Rust and
 * forwards clicks back. The only state kept here is view state (what is
 * expanded, which options are ticked, where the onboarding flow is).
 */

import { listen } from "@tauri-apps/api/event";

import { button, el } from "../shared/dom";
import {
  api,
  AWAY_EVENT,
  HOOKS_EVENT,
  GAZE_EVENT,
  type Gaze,
  GITHUB_EVENT,
  githubShown,
  type GithubStatus,
  HOVER_EVENT,
  type HooksStatus,
  ISLAND_CUTOUT_EVENT,
  type IslandCutout,
  ISLAND_PREFS_EVENT,
  type IslandPosition,
  type IslandPrefs,
  OPENING_CLOSED_EVENT,
  OUTSIDE_CLICK_EVENT,
  type PendingApproval,
  type SessionView,
  type Snapshot,
  STATE_EVENT,
  UPDATES_EVENT,
  type UpdateStatus,
  type Watched,
} from "../shared/ipc";
import type { Mascot } from "../shared/mascots";
import { fromRust, language, setLanguage, t } from "../shared/i18n";
import { playSound } from "../shared/sounds";
import { atlantisCard, atlantisSign, atlantisTabs, running, type Tab, tick } from "./atlantis";
import { approvalCard, pruneSelections, questionCard, sessionCard, stateLabel } from "./cards";
import { githubCard, shownAccounts } from "./github";
import { agentShort, agentTag, byUrgency, prIcon, rank, sessionKey, sessionTone, type Tone, topTone } from "./notch";
import { livelyMascot, onGaze, poke, react, setBusy, setLively } from "./lively";
import { agentNotice, awayLine, githubNotice, noticeRows, sayLine, type ViewNotice } from "./notices";
import { type Onboarding, onboardingCard } from "./onboarding";
import { askingUpdates, updateChip, updatesQuestion } from "./updates";

const MAX_HEIGHT = 440;
/** How long the island takes to open or fold. */
const MORPH_MS = 360;
/** Agent tags shown side by side; the rest becomes a counter. */
const MAX_TAGS = 4;
/** How long the pointer must be gone before an idle island folds. */
const HIDE_DELAY_MS = 700;
/** How long a new notice's phrase shows before the island folds back (#131). */
const SAY_MS = 5000;

const root = document.getElementById("island")!;

let snapshot: Snapshot = { sessions: [], approvals: [] };
let expanded = false;
let github: GithubStatus | null = null;
let updates: UpdateStatus | null = null;
let updatesError: string | null = null;
let mascot: Mascot = "eyes";
let position: IslandPosition = "top";
let cutout: IslandCutout | null = null;
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
/** Each session's last event when the user last clicked it, or the island. */
const seen = new Map<string, number>();
/** Each session's color from its state alone (not what was seen), to ring on a real change. */
const previousTones = new Map<string, Tone>();
/** Cards already rung for: a new card rings even when its session was amber already. */
const rungCards = new Set<string>();
/** Green (finished) or red (failed) sessions nobody clicked yet, kept after the session is gone. */
const unseen = new Map<string, ViewNotice>();
/** GitHub notices clicked away; forgotten once the notice goes, so its return is news. */
const seenNotices = new Set<string>();
/** Each account's GitHub notices already there; an account's first answer is not news. */
const knownNotices = new Map<string, Set<string>>();
/** GitHub's notices of the shown accounts, worked out once per render. */
let githubNow: ViewNotice[] = [];
/** Each session's state, to speak on a change that has a phrase. */
const previousStates = new Map<string, SessionView["state"]>();
/** The notice whose phrase the folded island shows for a few seconds (#131). */
let saying: ViewNotice | null = null;
/** Runs only while the phrase is drawn: started when it is. */
let sayTimer: number | undefined;
let tone: Tone = "gray";
/** The color just rose: the next mascot drawn jumps, once. */
let bounce = false;
/**
 * Nobody is looking (#157): the screen is locked, the mouse and the keyboard
 * rested ten minutes, or the computer slept. Nothing rings or speaks then.
 */
let away = false;
/** What came while nobody was looking, or while Cortex was closed. */
const kept = new Map<string, ViewNotice>();
/** The pink summary of what was kept, until See all or Dismiss (#157). */
const awaySummary = new Map<string, ViewNotice>();
/** The summary's notices as they stand, worked out once per render. */
let summaryNow: ViewNotice[] = [];
/** The summary's soft sound is owed: it rings once, when the summary first shows. */
let summaryOwed = false;
/** A card came while nobody was looking: its sound rings once somebody is back. */
let cardWhileAway = false;
/** The opening screen went: the island is in sight. */
let revealed = false;
/** The notch was drawn once since nobody is looking; it then waits as it is. */
let awayDrawn = false;
/** Opened by a click (the Atlantis sign, an Atlantis notice): open until a click outside (#173). */
let pinned = false;
/** The list's tab, once Atlantis has something to show; `null` until the user picks one. */
let tab: Tab | null = null;
/** The Atlantis card open: its pull request's URL; "" once the user closed it. */
let focus: string | null = null;
/** The card just picked comes into view once drawn. */
let scrollToFocus = false;
/** The Atlantis tab's pull requests, worked out once per render. */
let atlantisNow: Watched[] = [];
/** Ticks the running clocks while they are in sight (#173). */
let clockTimer: number | undefined;

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
    // The pink summary stays folded under the pointer: its buttons are there.
    (!(hovered || pinned) || showsSummary())
  );
}

const toneOf = (session: SessionView): Tone =>
  sessionTone(session, (seen.get(sessionKey(session)) ?? -1) >= session.last_event_ms);

/** Every GitHub notice of the shown accounts, once each, clicked away or not. */
function allGithubNotices(): ViewNotice[] {
  if (!github?.enabled) return [];
  const byKey = new Map<string, ViewNotice>();
  for (const { summary } of shownAccounts(github.accounts)) {
    for (const notice of summary.notices) {
      const view = githubNotice(notice);
      if (!byKey.has(view.key)) byKey.set(view.key, view);
    }
  }
  return [...byKey.values()];
}

/** Every notice standing, the most urgent first: agents', gone sessions' and GitHub's. */
function activeNotices(): ViewNotice[] {
  const live = new Set(snapshot.sessions.map(sessionKey));
  const notices = [
    ...snapshot.sessions.map((s) => agentNotice(s, toneOf(s))),
    ...[...unseen.values()].filter((n) => !live.has(n.key)),
    ...githubNow.filter((n) => !seenNotices.has(n.key)),
  ].filter((n): n is ViewNotice => n !== null);
  return notices.sort((a, b) => rank(b.tone) - rank(a.tone));
}

function stopSaying(): void {
  window.clearTimeout(sayTimer);
  sayTimer = undefined;
  saying = null;
}

/**
 * GitHub's notices that just appeared, the most urgent first. An account's
 * first answer (after launch, after turning GitHub on, or once it is shown
 * again) is not news; a notice that went and comes back is.
 */
function freshGithubNotices(): ViewNotice[] {
  githubNow = allGithubNotices();
  const standing = new Set(githubNow.map((n) => n.key));
  for (const key of seenNotices) if (!standing.has(key)) seenNotices.delete(key);
  const fresh: ViewNotice[] = [];
  const accounts = github?.enabled ? shownAccounts(github.accounts) : [];
  for (const login of knownNotices.keys()) {
    if (!accounts.some((a) => a.login === login)) knownNotices.delete(login);
  }
  for (const { login, summary, new_since_last_run: news } of accounts) {
    const keys = new Set(summary.notices.map((n) => githubNotice(n).key));
    const known = knownNotices.get(login);
    knownNotices.set(login, keys);
    if (!known) {
      // Not news, except what came while Cortex was closed: that is kept for the summary (#157).
      for (const notice of summary.notices) {
        const view = githubNotice(notice);
        if (news?.includes(notice.key) && !seenNotices.has(view.key)) kept.set(view.key, view);
      }
      continue;
    }
    for (const notice of summary.notices) {
      const view = githubNotice(notice);
      if (!known.has(view.key) && !seenNotices.has(view.key) && !fresh.some((f) => f.key === view.key)) fresh.push(view);
    }
  }
  return fresh.sort((a, b) => rank(b.tone) - rank(a.tone));
}

/** The island's color: the most urgent of the live sessions, the unseen ones and GitHub's notices. */
function updateTone(): void {
  const live = new Set<string>();
  let notice: Tone = "gray";
  const spoken: ViewNotice[] = [];
  for (const session of snapshot.sessions) {
    const key = sessionKey(session);
    live.add(key);
    const t = toneOf(session);
    const state = sessionTone(session, false);
    if (state !== previousTones.get(key)) notice = topTone([notice, state]);
    previousTones.set(key, state);
    const view = agentNotice(session, t);
    // A phrase on a change of state that has one, even within one color.
    if (view && session.state !== previousStates.get(key)) spoken.push(view);
    previousStates.set(key, session.state);
    if (view && (t === "green" || t === "red")) unseen.set(key, view);
    else unseen.delete(key);
  }
  for (const key of seen.keys()) if (!live.has(key)) seen.delete(key);
  for (const key of previousTones.keys()) if (!live.has(key)) previousTones.delete(key);
  for (const key of previousStates.keys()) if (!live.has(key)) previousStates.delete(key);
  const cards = new Set(snapshot.approvals.map((a) => a.request_id));
  const newCard = [...cards].some((id) => !rungCards.has(id));
  if (newCard) notice = topTone([notice, "amber"]);
  for (const id of rungCards) if (!cards.has(id)) rungCards.delete(id);
  for (const id of cards) rungCards.add(id);
  if (away) cardWhileAway ||= newCard;
  else if (soundReady && sound && notice !== "gray") playSound(notice);
  const fromGithub = freshGithubNotices();
  const next = topTone([
    ...snapshot.sessions.map(toneOf),
    ...[...unseen.values()].map((n) => n.tone),
    ...githubNow.filter((n) => !seenNotices.has(n.key)).map((n) => n.tone),
  ]);
  if (topTone([next, tone]) !== tone && next !== tone) bounce = true;
  tone = next;
  if (away) {
    // Nobody is looking (#157): what came waits for the summary, without a sound or a phrase.
    for (const view of [...spoken, ...fromGithub]) kept.set(view.key, view);
  } else if (kept.size > 0) {
    for (const [key, view] of kept) awaySummary.set(key, view);
    kept.clear();
    summaryOwed = true;
  }
  // Only what is as urgent as the island now speaks; GitHub's ring only then.
  const top = away ? undefined : [...spoken, ...fromGithub].sort((a, b) => rank(b.tone) - rank(a.tone))[0];
  if (soundReady && top && rank(top.tone) >= rank(tone)) {
    if (sound && fromGithub.includes(top)) playSound(top.tone);
    if (!saying || rank(top.tone) >= rank(saying.tone)) {
      stopSaying();
      saying = top;
    }
  }
  // A phrase whose notice went, or that a more urgent color now covers, stops.
  if (saying && (rank(saying.tone) < rank(tone) || !activeNotices().some((n) => n.key === saying?.key))) stopSaying();
}

/**
 * The summary as it stands (#157): a notice clicked away or gone leaves it,
 * and each one is taken as it is now (an agent done, then failed, is red).
 */
function refreshSummary(): void {
  if (awaySummary.size > 0) {
    const standing = new Map(activeNotices().map((n) => [n.key, n]));
    for (const key of [...awaySummary.keys()]) {
      const now = standing.get(key);
      if (now) awaySummary.set(key, now);
      else awaySummary.delete(key);
    }
  }
  if (awaySummary.size === 0) summaryOwed = false;
  summaryNow = [...awaySummary.values()];
}

/** The island is pink with the summary: a waiting card comes first (#157). */
function showsSummary(): boolean {
  return snapshot.approvals.length === 0 && summaryNow.length > 0;
}

/** The summary's soft sound; under a louder one it waits for that one to end. */
function ringSummary(): void {
  if (playSound("pink")) return;
  window.setTimeout(() => {
    if (sound && !away && !saying && showsSummary() && isHidden()) playSound("pink");
  }, 2500);
}

/** "See all" on the summary: the list opens, each notice in its color, and the pink goes. */
function seeAll(): void {
  awaySummary.clear();
  summaryOwed = false;
  if (hovered) render();
  else setHovered(true);
}

/** "Dismiss" on the summary: everything counts as seen, as a click on the island. */
function dismissSummary(): void {
  awaySummary.clear();
  summaryOwed = false;
  acknowledge();
  // Gone, not opened: the island stays folded until the pointer comes back.
  window.clearTimeout(hideTimer);
  hideTimer = undefined;
  hovered = false;
  render();
}

/** A click on the island: every notice goes back to gray, GitHub's included. */
function acknowledge(): void {
  for (const session of snapshot.sessions) seen.set(sessionKey(session), session.last_event_ms);
  unseen.clear();
  for (const notice of githubNow) seenNotices.add(notice.key);
  stopSaying();
}

/** "Clear all" under the list: every notice goes, as on a click on the island. */
function clearAll(): void {
  acknowledge();
  render();
}

/** Opened by a click, the island stays open until a click outside; Rust reports that click (#173). */
function setPinned(on: boolean): void {
  if (pinned === on) return;
  pinned = on;
  // Unpinned in Rust, an outside click would never come: fold on hover out instead.
  void api.islandPin(on).catch(() => { if (on) pinned = false; });
}

/** The Atlantis tab on a pull request's card, open until a click outside (#173). */
function openAtlantis(url: string | null): void {
  setPinned(true);
  // Out of the session cards; a waiting card still comes first.
  if (snapshot.approvals.length === 0) {
    expanded = false;
    autoExpanded = false;
  }
  tab = "atlantis";
  focus = url;
  scrollToFocus = true;
  render();
}

/** A click on an Atlantis notice: that one goes, and its pull request's card opens (#173); without a card, GitHub. */
function goToCard(notice: ViewNotice): void {
  const url = notice.url ?? null;
  if (!atlantisCards(url).some((w) => w.pull.url === url)) {
    actOn(notice);
    return;
  }
  if (saying?.key === notice.key) stopSaying();
  seenNotices.add(notice.key);
  openAtlantis(url);
}

/** At most this many cards in the Atlantis tab, as each account sends them. */
const MAX_CARDS = 8;

/** The Atlantis tab's pull requests, of every account shown, each once, in the core's order. */
function watchedNow(): Watched[] {
  if (!github?.enabled) return [];
  const byUrl = new Map<string, Watched>();
  for (const { summary } of shownAccounts(github.accounts)) {
    for (const watched of summary.atlantis) if (!byUrl.has(watched.pull.url)) byUrl.set(watched.pull.url, watched);
  }
  return [...byUrl.values()].sort((a, b) => a.order - b.order || b.at.localeCompare(a.at)).slice(0, MAX_CARDS);
}

/** The sign's click: the first run going now, read at the click, not when drawn. */
function openFirstRun(): void {
  openAtlantis(atlantisNow.find((w) => running(w.pull.plan))?.pull.url ?? null);
}

/** The pull requests in the Atlantis tab, with the one a notice opened even when it moved over an hour ago. */
function atlantisCards(pick = focus): Watched[] {
  if (!pick || atlantisNow.some((w) => w.pull.url === pick)) return atlantisNow;
  for (const { summary } of github?.enabled ? shownAccounts(github.accounts) : []) {
    const own = summary.mine.find((p) => p.url === pick);
    const pull = own ?? summary.review_requests.find((p) => p.url === pick);
    if (pull?.plan) return [{ pull, own: own !== undefined, merged: false, order: 2, at: "" }, ...atlantisNow];
  }
  return atlantisNow;
}

/** A link in an Atlantis card: the pull request, its plan's comment, a live log. */
function openLink(url: string): void {
  void api.openUrl(url).catch(() => { /* Nothing to undo. */ });
}

/** The running clocks tick only while in sight and somebody looks (#51). */
function setClock(on: boolean): void {
  if (on && clockTimer === undefined) clockTimer = window.setInterval(() => tick(root), 1000);
  if (!on && clockTimer !== undefined) {
    window.clearInterval(clockTimer);
    clockTimer = undefined;
  }
}

/** A click on one notice in the list: that one goes, and GitHub's open. */
function actOn(notice: ViewNotice): void {
  if (saying?.key === notice.key) stopSaying();
  if (notice.url) {
    seenNotices.add(notice.key);
    void api.openUrl(notice.url).catch(() => { /* The notice is cleared either way. */ });
    render();
    return;
  }
  const session = snapshot.sessions.find((s) => sessionKey(s) === notice.key);
  if (session) {
    acknowledgeSession(session);
  } else {
    unseen.delete(notice.key);
    render();
  }
}

/** A click on a session's card: that session goes back to gray. */
function acknowledgeSession(session: SessionView): void {
  const before = toneOf(session);
  const key = sessionKey(session);
  seen.set(key, session.last_event_ms);
  unseen.delete(key);
  // A redraw for nothing would drop a text selection or cut the mascot's jump.
  if (toneOf(session) !== before) render();
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

/** The "+N" beside the phrase. */
function othersStanding(): number {
  return activeNotices().filter((n) => n.key !== saying?.key).length;
}

/** Everything the folded notch shows: the same key means nothing to repaint. */
function foldedKey(): string {
  const { reviews, failing } = prTotals();
  const say = saying ? `${saying.key}|${saying.detail}|${othersStanding()}` : "";
  const pink = !saying && showsSummary() ? summaryNow.map((n) => n.tone).join() : "";
  // The Atlantis sign turns until nobody looks (#173): then it is drawn once more, still.
  const runs = atlantisNow.filter((w) => running(w.pull.plan)).length;
  const atlantis = runs > 0 ? `${runs}${away ? " still" : ""}` : "";
  return `${tone}|${mascot}|${position}|${reviews}|${failing}|${bounce}|${cutout?.width}|${cutout?.height}|${say}|${pink}|${language()}|${atlantis}`;
}

/** The folded island is pink while the summary stands, unless a phrase speaks over it (#157). */
function shownTone(open: boolean): Tone | "pink" {
  return !open && !saying && showsSummary() ? "pink" : tone;
}

/** Touch the class only when it changes: any DOM change costs a repaint (#51). */
function setClass(open: boolean): void {
  // Away, nothing turns (#51, #173): a turning ring repaints on every frame.
  const name = `island tone-${shownTone(open)}${open ? " island--open" : ""}${cutout ? " island--cutout" : ""}${away ? " island--away" : ""}`;
  if (root.className !== name) root.className = name;
  if (root.dataset.position !== position) root.dataset.position = position;
}

// ---------------------------------------------------------------------------
// Rendering

function reveal(): void {
  if (revealed) return;
  revealed = true;
  document.body.classList.remove("waiting");
  // A summary kept for this moment rings now (#157).
  if (summaryOwed) queueMicrotask(render);
}

function render(): void {
  // A card waits on the user: it cannot wait for the opening screen.
  if (snapshot.approvals.length > 0) reveal();
  const pending = new Set(snapshot.approvals.map((a) => a.request_id));
  dismissed = new Set([...dismissed].filter((id) => pending.has(id)));
  pruneSelections(pending);
  updateTone();
  refreshSummary();
  setBusy(tone !== "gray" || awaySummary.size > 0 || snapshot.sessions.some((s) => s.state === "working" || s.state === "thinking"));
  atlantisNow = watchedNow();

  if (isHidden()) {
    expanded = false;
    autoExpanded = false;
    // A list opened again starts on its notices and its first card (#173).
    tab = null;
    focus = null;
    setPinned(false);
    setClock(false);
    // The phrase's seconds count from when it is drawn.
    if (saying && sayTimer === undefined) {
      sayTimer = window.setTimeout(() => {
        sayTimer = undefined;
        saying = null;
        render();
      }, SAY_MS);
    }
    const pink = !saying && showsSummary();
    if (pink && summaryOwed && revealed && soundReady && !away) {
      // Back, and something came (#157): one soft sound, once, and the mascot jumps.
      summaryOwed = false;
      bounce = true;
      if (sound) ringSummary();
    }
    // Same notch: paint nothing. Each paint with the screen locked keeps WebKit
    // retrying to free its buffers, which costs ~1% CPU until unlock (#51).
    // Nobody looking, it is drawn once more (a phrase goes) and then waits as
    // it is until somebody is back (#157).
    const key = foldedKey();
    if (away && folded !== null && (awayDrawn || folded === key)) {
      awayDrawn = true;
      return;
    }
    if (folded === key) return;
    awayDrawn = away;
    const before = sizeBefore();
    const fade = folded === null;
    setClass(false);
    const node = el("div", saying || pink ? "notch notch--say" : "notch");
    const left = mascotNode();
    const totals = prTotals();
    const right = totals.reviews > 0 || totals.failing > 0 ? [prCount(totals)] : [];
    // Atlantis running (#173): its sign beside the pull requests, a click opens its tab.
    const runs = atlantisNow.filter((w) => running(w.pull.plan)).length;
    if (runs > 0) right.push(atlantisSign(runs, openFirstRun));
    // A new notice says what happened for a few seconds, and the pink summary
    // stays until a click (#157); beside the camera, on a row under it.
    const say = saying ? sayLine(saying, othersStanding()) : pink ? awayLine(summaryNow, seeAll, dismissSummary) : [];
    let top: HTMLElement = node;
    if (cutout) {
      splitAtCamera(node, [left], right);
      if (saying || pink) {
        top = el("div");
        const below = el("div", "say-row");
        below.append(...say);
        top.append(node, below);
      }
    } else {
      node.append(left, ...(saying || pink ? say : right));
    }
    top.addEventListener("click", (event) => {
      pokeUnlessButton(event);
      // On the summary, a click shows the list rather than clearing it.
      if (pink) {
        seeAll();
        return;
      }
      acknowledge();
      setHovered(true);
    });
    folded = foldedKey();
    root.replaceChildren(top);
    void fitWindowToContent(before, fade);
    return;
  }
  const fade = folded !== null;
  folded = null;
  // Open, the list shows every notice: no phrase on top of it.
  stopSaying();

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
  const before = sizeBefore();
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
  // A card just picked (the sign, a notice) comes into view.
  if (scrollToFocus) {
    scrollToFocus = false;
    for (const card of root.querySelectorAll<HTMLElement>(".atl-card")) if (card.dataset.url === focus) card.scrollIntoView({ block: "nearest" });
  }
  setClock(!away && root.querySelector("[data-since]") !== null);
  void fitWindowToContent(before, fade);
}

/**
 * The mascot in the island's color, or a dot when the user hid it. The jump
 * is spent on the first mascot drawn after the color rose: a redraw never
 * starts it again, so it cannot loop. The mascot is one node across redraws,
 * so the lively mascot keeps its eyes (#145).
 */
function mascotNode(): Element {
  const svg = livelyMascot(mascot);
  const node: Element = svg ?? el("span", "brand-dot");
  if (bounce) {
    bounce = false;
    const shown = shownTone(!isHidden());
    if (svg && shown !== "gray") react(shown);
    else node.classList.add("bounce");
  }
  return node;
}

/** A click on the island, not on one of its buttons, is a poke for the mascot. */
function pokeUnlessButton(event: Event): void {
  if (!(event.target instanceof Element && event.target.closest("button"))) poke();
}

function prCount({ reviews, failing }: { reviews: number; failing: number }): HTMLElement {
  const node = el("span", "pr-count");
  const notes = [t("island.prs", reviews)];
  node.append(prIcon());
  if (reviews > 0 || failing === 0) node.append(el("span", undefined, String(reviews)));
  if (failing > 0) {
    node.append(el("span", "pr-failing"));
    notes.push(t("island.failing", failing));
  }
  node.title = notes.join(" · ");
  return node;
}

/**
 * A click outside folds the island, cards included (the notch stays amber
 * while they wait), and an island opened by a click (#173).
 */
function foldAway(): void {
  if (snapshot.approvals.length === 0 && !pinned) return;
  for (const approval of snapshot.approvals) dismissed.add(approval.request_id);
  setPinned(false);
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

function toggle(): void {
  expanded = !expanded;
  autoExpanded = false;
  render();
}

function setCutout(value: IslandCutout | null): void {
  cutout = value;
  if (value) {
    root.style.setProperty("--cutout-width", `${value.width}px`);
    root.style.setProperty("--cutout-height", `${value.height}px`);
  }
}

function splitAtCamera(node: HTMLElement, left: Element[], right: Element[]): void {
  const leftSide = el("div", "cutout-left");
  const rightSide = el("div", "cutout-right");
  leftSide.append(...left);
  rightSide.append(...right);
  node.classList.add("cutout-row");
  node.append(leftSide, rightSide);
}

/** Agent tags around the mascot, the most urgent first; the PR count; chips and the gear. */
function bar(): HTMLElement {
  const node = el("div", "bar");
  const sessions = byUrgency(snapshot.sessions, toneOf);
  const chips = updateChips();
  // An update chip takes the room of two tags: the bar never outgrows the window.
  // Beside the camera the right side only has room for the counter, the PR
  // count and the gear: tags stay on the left, one when "1 of N" is there too.
  const room = cutout ? (snapshot.approvals.length > 1 ? 1 : 2) : chips.length > 0 ? 2 : MAX_TAGS;
  const tags = sessions.slice(0, room).map((session) => agentTag(session, toneOf(session), sessionLabel(session)));
  const center = el("span", "bar-center");
  center.append(mascotNode());
  if (snapshot.approvals.length > 1) center.append(el("span", "queue", t("island.queue", snapshot.approvals.length)));
  const right = tags.slice(2);
  if (sessions.length > room) right.push(el("span", "more", `+${sessions.length - room}`));

  const accounts = github?.enabled ? shownAccounts(github.accounts) : [];
  if (accounts.length > 0) {
    const pr = button("btn-pr", "", toggle);
    pr.append(prCount(prTotals()));
    right.push(pr);
  }
  const runs = atlantisNow.filter((w) => running(w.pull.plan)).length;
  if (runs > 0) right.push(atlantisSign(runs, openFirstRun));
  if (hooksInstalled === false && onboarding.phase === "closed" && sessions.length === 0) {
    chips.unshift(button("btn-chip", t("island.connect"), () => void openOnboarding()));
  }
  // Beside the camera the chips get a row of their own below it.
  if (!cutout) right.push(...chips);
  right.push(settingsButton());
  const firstTags = tags.slice(0, 2);
  if (cutout) splitAtCamera(node, [center, ...firstTags], right);
  else node.append(...firstTags, center, ...right);
  if (!cutout || chips.length === 0) return onClick(node, sessions.length > 0);
  const wrap = el("div");
  const below = el("div", "chip-row");
  below.append(...chips);
  wrap.append(node, below);
  return onClick(wrap, sessions.length > 0);
}

/**
 * Any click on the bar is a click on the island: it clears the colors, even
 * on a button (seen while capturing, before the button stops it). A click
 * beside the buttons opens or closes the session cards.
 */
function onClick(top: HTMLElement, toggles: boolean): HTMLElement {
  top.addEventListener("click", (event) => {
    // The Atlantis sign opens its tab and clears nothing (#173).
    if (event.target instanceof Element && event.target.closest(".atl-sign")) return;
    acknowledge();
    pokeUnlessButton(event);
    // A button stops the click and may not redraw: the cleared colors still must.
    queueMicrotask(render);
  }, { capture: true });
  top.addEventListener("click", () => (toggles ? toggle() : render()));
  return top;
}

function sessionLabel(session: SessionView): string {
  return session.current ?? fromRust(session.ticker.at(-1)?.label ?? stateLabel(session.state));
}

/** Never offered over a card: restarting would drop the agent's waiting request. */
function updateChips(): HTMLElement[] {
  if (snapshot.approvals.length > 0) return [];
  return updateChip(updates, () => void installUpdate());
}

function settingsButton(): HTMLButtonElement {
  const node = button("btn-icon", "\u2699\uFE0E", () => void api.openSettings());
  node.title = t("island.settings");
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
      el("span", "next-text", approval.questions.length > 0 ? t("island.asks") : t("island.wants", approval.tool_name, approval.detail)),
    );
    node.append(row);
  }
  if (rest.length > 3) node.append(el("div", "next-row next-more", t("island.more", rest.length - 3)));
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
  if (!expanded) {
    // Hovering, or opened by a click (#173), lists what is standing, the most
    // urgent first (#131); with Atlantis to follow, in two tabs.
    const listing = (hovered || pinned) && snapshot.approvals.length === 0;
    const notices = listing ? activeNotices() : [];
    const cards = listing ? atlantisCards() : [];
    if (notices.length + cards.length === 0) return question;
    const rows = noticeRows(notices, { act: actOn, clearAll, plan: goToCard });
    if (cards.length === 0) return [...question, rows];
    const shown: Tab = tab ?? (notices.length > 0 ? "notices" : "atlantis");
    const runs = cards.filter((w) => running(w.pull.plan)).length;
    const tabs = atlantisTabs(shown, notices.length, runs, (picked) => {
      tab = picked;
      render();
    });
    if (shown === "notices") return [...question, tabs, notices.length > 0 ? rows : el("div", "atl-empty", t("atl.empty"))];
    // A pick that left the list falls back to the first card; "" keeps them all closed.
    const open = focus === "" ? null : cards.some((w) => w.pull.url === focus) ? focus : (cards[0]?.pull.url ?? null);
    const list = el("div", "atl-cards");
    const actions = {
      open: openLink,
      toggle: (url: string) => {
        focus = url === open ? "" : url;
        render();
      },
    };
    for (const watched of cards) list.append(atlantisCard(watched, watched.pull.url === open, actions));
    return [...question, tabs, list];
  }

  const accounts = github?.enabled ? shownAccounts(github.accounts) : [];
  const sessions = snapshot.sessions.map((session) => {
    const card = sessionCard(session);
    card.addEventListener("click", () => acknowledgeSession(session));
    return card;
  });
  return [...question, ...cardsInView(), ...sessions, ...(accounts.length > 0 ? [githubCard(accounts)] : [])];
}

const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
type Size = { width: number; height: number };
/** The island easing between two sizes, and where it is going. */
let morph: { animation: Animation; from: Size; to: Size } | undefined;
/** Bumped by every fit: a newer one takes over a morph mid-way. */
let fitTurn = 0;
/** The size last fitted; none yet: the first fit shows the island at once. */
let lastFit: Size | undefined;
/** The size last asked of the window: the asks run in order, so it ends there. */
let asked: Size | undefined;
/** Asks not answered yet: until then the window may still pass through each. */
const asking: Size[] = [];

async function fitWindow(size: Size): Promise<void> {
  asked = size;
  asking.push(size);
  try {
    await api.islandFit(size.width, size.height);
  } finally {
    asking.splice(asking.indexOf(size), 1);
  }
}

const fitSize = (rect: DOMRect): Size => ({
  width: Math.ceil(rect.width),
  height: Math.min(MAX_HEIGHT, Math.ceil(rect.height)),
});
const sameSize = (a: Size, b: Size): boolean => a.width === b.width && a.height === b.height;
const holds = (a: Size, b: Size): boolean => a.width >= b.width && a.height >= b.height;
/** What the island shows before a redraw: measured only while it eases. */
const sizeBefore = (): Size | undefined => (morph ? fitSize(root.getBoundingClientRect()) : lastFit);

/**
 * Size the window to what is drawn, so no invisible area eats clicks. A
 * change of size eases (#118): the window first grows to hold both sizes,
 * the island eases from `before` to its new size (its content already laid
 * out at the final width, so nothing rewraps), and the window then shrinks
 * to fit. A redraw that keeps the destination (an agent event mid-way)
 * carries on from where the ease was. Nothing animates at rest.
 */
async function fitWindowToContent(before: Size | undefined, fade: boolean): Promise<void> {
  const turn = ++fitTurn;
  const running = morph;
  const elapsed = running?.animation.currentTime;
  const started = running?.animation.startTime;
  running?.animation.cancel();
  morph = undefined;
  root.classList.remove("island--morphing");
  const to = fitSize(root.getBoundingClientRect());
  const carryOn = running !== undefined && sameSize(running.to, to) && typeof elapsed === "number";
  const from = carryOn ? running.from : before;
  if (from === undefined || reducedMotion.matches || sameSize(from, to)) {
    lastFit = to;
    await fitWindow(to);
    return;
  }
  const inner = root.firstElementChild?.getBoundingClientRect().width ?? to.width;
  root.style.setProperty("--morph-inner", `${inner}px`);
  root.classList.add("island--morphing");
  const animation = root.animate(
    [{ width: `${from.width}px`, height: `${from.height}px` }, { width: `${to.width}px`, height: `${to.height}px` }],
    { duration: MORPH_MS, easing: "cubic-bezier(0.32, 0.72, 0, 1)", fill: "both" },
  );
  const grown = { width: Math.max(from.width, to.width), height: Math.max(from.height, to.height) };
  // Held where it is until the window can show the new size. A window that
  // holds it already (a fold, a redraw mid-way), and is not on its way to a
  // smaller size, lets it run on: every hold costs a frame, and busy agents
  // redraw many times in one ease (#171).
  const ready =
    asked !== undefined &&
    holds(asked, grown) &&
    asking.every((size) => holds(size, grown)) &&
    holds({ width: window.innerWidth, height: window.innerHeight }, grown);
  if (!ready) animation.pause();
  // A new ease stands still for a frame before it moves: on the same start
  // it goes on as if it had never stopped.
  if (carryOn && ready && typeof started === "number") animation.startTime = started;
  else if (carryOn) animation.currentTime = elapsed;
  morph = { animation, from, to };
  // Opening or folding, the new content comes in softly (it goes with its
  // nodes on the next render); a redraw of what is shown must not blink.
  if (fade) {
    for (const child of root.children) {
      child.animate([{ opacity: 0 }, { opacity: 1 }], { duration: MORPH_MS * 0.7, delay: MORPH_MS * 0.15, easing: "ease-out", fill: "backwards" });
    }
  }
  try {
    if (!ready) {
      await fitWindow(grown);
      // The page learns its new size a frame or so after the window: easing
      // before that would push the island off its edge for a frame.
      for (let i = 0; i < 3 && (window.innerWidth < grown.width || window.innerHeight < grown.height); i++) {
        await new Promise(requestAnimationFrame);
      }
      if (turn !== fitTurn) return;
      animation.play();
    }
    await animation.finished;
  } catch {
    // Cancelled by a newer change, which then owns the window; or the IPC
    // failed, and the island must not stay held at its old size.
    if (turn !== fitTurn) return;
  }
  if (turn !== fitTurn) return;
  animation.cancel();
  morph = undefined;
  lastFit = to;
  root.classList.remove("island--morphing");
  await fitWindow(to).catch(() => { /* The next redraw fits it again. */ });
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
    await api.updatesStart();
  } catch {
    // Only refused over a card, and the chip is not offered then.
  }
}

// ---------------------------------------------------------------------------

async function start(): Promise<void> {
  let awayChanged = false;
  await listen<boolean>(AWAY_EVENT, ({ payload }) => {
    awayChanged = true;
    away = payload;
    if (away) {
      // The notch is drawn once more without the phrase, then waits. An
      // island left open by a click folds: open, every event redraws it (#51).
      stopSaying();
      awayDrawn = false;
      if (pinned) {
        setPinned(false);
        window.clearTimeout(hideTimer);
        hideTimer = undefined;
        hovered = false;
      }
    } else if (cardWhileAway) {
      cardWhileAway = false;
      // Back with an agent waiting: its card comes first, and rings once (#157).
      if (snapshot.approvals.length > 0 && soundReady && sound) playSound("amber");
    }
    render();
  });
  // Before any notice is heard: one that comes while nobody looks is kept (#157).
  const awayAtLaunch = await api.awayNow().catch(() => false);
  if (!awayChanged) away = awayAtLaunch;
  // The island starts hidden (index.html) and appears once this launch's
  // opening screen goes (#152); the event can come before the answer. Never
  // hidden past 12 s, whatever that screen does.
  let openingDone = false;
  await listen(OPENING_CLOSED_EVENT, () => {
    openingDone = true;
    reveal();
  });
  const opening = await api.openingShown().catch(() => false);
  if (!opening || openingDone) reveal();
  else window.setTimeout(reveal, 12_000);
  let githubChanged = false;
  await listen<GithubStatus>(GITHUB_EVENT, (event) => {
    githubChanged = true;
    // A redraw under the pointer can lose a click: only for a change.
    const same = githubShown(github) === githubShown(event.payload);
    github = event.payload;
    if (!same) render();
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
  await listen<IslandPrefs>(ISLAND_PREFS_EVENT, ({ payload }) => {
    islandChanged = true;
    sound = payload.sound;
    setLively(payload.lively);
    const relabel = payload.language !== language();
    setLanguage(payload.language);
    if (relabel) {
      // Notices kept from before carry their phrase: made again in this language.
      for (const map of [unseen, kept, awaySummary]) for (const [key, view] of map) map.set(key, view.again());
      if (saying) saying = saying.again();
    }
    if (!relabel && mascot === payload.mascot && position === payload.position) return;
    mascot = payload.mascot;
    position = payload.position;
    render();
  });
  let cutoutChanged = false;
  await listen<IslandCutout | null>(ISLAND_CUTOUT_EVENT, ({ payload }) => {
    cutoutChanged = true;
    setCutout(payload);
    render();
  });
  await listen<boolean>(HOVER_EVENT, (event) => setHovered(event.payload));
  await listen<Gaze>(GAZE_EVENT, ({ payload }) => onGaze(payload));
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
  const [initialSnapshot] = await Promise.all([api.snapshot(), refreshHooksStatus(), api.githubStatus().then((value) => { if (!githubChanged) github = value; }).catch(() => { /* GitHub is optional. */ }), api.updatesStatus().then((value) => { if (!updatesChanged) updates = value; }).catch(() => { /* Without a status the question waits for the next event. */ }), api.islandPrefs().then((value) => { if (!islandChanged) { ({ mascot, sound, position } = value); setLively(value.lively); setLanguage(value.language); } }).catch(() => { /* Keep the island defaults. */ }), api.islandCutout().then((value) => { if (!cutoutChanged) setCutout(value); }).catch(() => { /* Keep the regular layout. */ })]);
  if (!snapshotChanged) snapshot = initialSnapshot;
  render();
  soundReady = true;
  // What came while Cortex was closed may be in already (#157).
  if (summaryOwed) render();
}

void start();
