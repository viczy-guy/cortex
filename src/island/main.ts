/**
 * The island. Dumb by design: it draws the latest snapshot from Rust and
 * forwards clicks back. The only state kept here is view state (what is
 * expanded, which options are ticked, where the onboarding flow is).
 */

import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

import { button, el } from "../shared/dom";
import {
  api,
  HOOKS_EVENT,
  type HooksStatus,
  type SessionView,
  type Snapshot,
  STATE_EVENT,
} from "../shared/ipc";
import { approvalCard, pruneSelections, questionCard, sessionCard, stateLabel } from "./cards";
import { type Onboarding, onboardingCard } from "./onboarding";

/** Must match `ISLAND_WIDTH` in Rust and the window width in tauri.conf.json. */
const WINDOW_WIDTH = 460;
const MAX_HEIGHT = 440;

const root = document.getElementById("island")!;

let snapshot: Snapshot = { sessions: [], approvals: [] };
let expanded = false;
/** True when the panel was opened by an approval, not by the user. */
let autoExpanded = false;
/** `null` until Rust answers; the "Connect" chip shows only on a definite `false`. */
let hooksInstalled: boolean | null = null;
let onboarding: Onboarding = { phase: "closed" };

// ---------------------------------------------------------------------------
// Rendering

function render(): void {
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
  pruneSelections(new Set(snapshot.approvals.map((a) => a.request_id)));

  root.replaceChildren(bar());
  const panel = panelContent();
  if (panel.length > 0) {
    const node = el("div", "panel");
    node.append(...panel);
    root.append(node);
  }
  void fitWindowToContent();
}

function bar(): HTMLElement {
  const node = el("div", "bar");
  node.append(el("span", "brand-dot"));

  if (snapshot.sessions.length === 0 && snapshot.approvals.length === 0) {
    node.append(el("span", "bar-idle", "cortex"));
    if (hooksInstalled === false && onboarding.phase === "closed") {
      node.append(button("btn-chip", "Connect Claude Code", () => void openOnboarding()));
    }
    node.append(settingsButton());
    return node;
  }

  for (const session of snapshot.sessions.slice(0, 3)) {
    node.append(pill(session));
  }
  node.append(settingsButton());
  node.addEventListener("click", () => {
    expanded = !expanded;
    autoExpanded = false;
    render();
  });
  return node;
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
  if (!expanded) return [];

  const cards = snapshot.approvals.map((approval) =>
    approval.questions.length > 0 ? questionCard(approval, render) : approvalCard(approval),
  );
  return [...cards, ...snapshot.sessions.map(sessionCard)];
}

/** Size the window to what is drawn, so no invisible area eats clicks. */
async function fitWindowToContent(): Promise<void> {
  const height = Math.min(MAX_HEIGHT, Math.ceil(root.getBoundingClientRect().height) + 8);
  await getCurrentWindow().setSize(new LogicalSize(WINDOW_WIDTH, height));
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

async function start(): Promise<void> {
  await listen<HooksStatus>(HOOKS_EVENT, (event) => {
    hooksInstalled = event.payload.installed;
    render();
  });
  await listen<Snapshot>(STATE_EVENT, (event) => {
    snapshot = event.payload;
    render();
  });
  [snapshot] = await Promise.all([api.snapshot(), refreshHooksStatus()]);
  render();
}

void start();
