/**
 * The island renderer. Dumb by design: it draws the latest snapshot from Rust
 * and forwards clicks back. No business logic lives here.
 *
 * Security: all dynamic data is set via `textContent` — never innerHTML.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

import {
  type PendingApproval,
  type SessionView,
  type Snapshot,
  STATE_EVENT,
  type UiDecision,
} from "../shared/ipc";

const WINDOW_WIDTH = 460;
const BAR_HEIGHT = 50;
const PANEL_MAX_HEIGHT = 400;

const root = document.getElementById("island")!;
let expanded = false;
let lastSnapshot: Snapshot = { sessions: [], approvals: [] };

// ---------------------------------------------------------------------------
// Rendering

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function render(snap: Snapshot): void {
  lastSnapshot = snap;
  const hasApprovals = snap.approvals.length > 0;
  // An approval demands eyes: force the panel open.
  if (hasApprovals) expanded = true;

  root.replaceChildren(renderBar(snap));
  if (expanded && (snap.sessions.length > 0 || hasApprovals)) {
    root.append(renderPanel(snap));
  }
  void resizeToContent();
}

function renderBar(snap: Snapshot): HTMLElement {
  const bar = el("div", "bar");
  bar.append(el("span", "brand-dot"));

  if (snap.sessions.length === 0 && snap.approvals.length === 0) {
    bar.append(el("span", "bar-idle", "cortex"));
    return bar;
  }

  for (const session of snap.sessions.slice(0, 3)) {
    bar.append(renderPill(session));
  }
  bar.addEventListener("click", () => {
    expanded = !expanded;
    render(lastSnapshot);
  });
  return bar;
}

function renderPill(session: SessionView): HTMLElement {
  const pill = el("div", "pill");
  pill.append(el("span", `state-dot state-${session.state}`));
  pill.append(el("span", "agent", session.key.agent));
  const label =
    session.current ??
    session.ticker.at(-1)?.label ??
    stateLabel(session.state);
  pill.append(el("span", "ticker", label));
  return pill;
}

function renderPanel(snap: Snapshot): HTMLElement {
  const panel = el("div", "panel");
  for (const approval of snap.approvals) {
    panel.append(renderApproval(approval));
  }
  for (const session of snap.sessions) {
    panel.append(renderSession(session));
  }
  return panel;
}

function renderSession(session: SessionView): HTMLElement {
  const card = el("div", "card");

  const head = el("div", "card-head");
  head.append(el("span", `state-dot state-${session.state}`));
  head.append(el("span", "title", session.key.agent));
  head.append(el("span", "meta", stateLabel(session.state)));
  card.append(head);

  if (session.ticker.length > 0) {
    const steps = el("div", "steps");
    for (const step of session.ticker.slice(-4)) {
      steps.append(el("div", "step", step.label));
    }
    card.append(steps);
  }

  if (session.changes.length > 0) {
    const changes = el("div", "changes");
    for (const change of session.changes.slice(-6)) {
      const row = el("div", "change");
      row.append(el("span", "add", `+${change.added}`));
      row.append(el("span", "del", `−${change.removed}`));
      row.append(el("span", "path", change.path));
      changes.append(row);
    }
    card.append(changes);
  }

  return card;
}

function renderApproval(approval: PendingApproval): HTMLElement {
  const card = el("div", "card approval");

  const head = el("div", "card-head");
  head.append(el("span", "state-dot state-waiting_approval"));
  head.append(el("span", "title", `${approval.tool_name} needs approval`));
  head.append(el("span", "meta", approval.session.agent));
  card.append(head);

  if (approval.detail) {
    card.append(el("div", "detail", approval.detail));
  }

  const actions = el("div", "actions");
  const button = (cls: string, label: string, decision: UiDecision) => {
    const b = el("button", `btn ${cls}`, label);
    b.addEventListener("click", () => void respond(approval.request_id, decision));
    return b;
  };
  actions.append(button("btn-deny", "Deny", "deny"));
  actions.append(button("btn-allow", "Allow", "allow"));
  actions.append(button("btn-neutral", "Always", "always"));
  const terminal = el("span", "terminal-link", "answer in terminal");
  terminal.addEventListener("click", () => void respond(approval.request_id, "ask"));
  actions.append(terminal);
  card.append(actions);

  return card;
}

function stateLabel(state: SessionView["state"]): string {
  switch (state) {
    case "idle": return "idle";
    case "thinking": return "thinking…";
    case "working": return "working";
    case "waiting_approval": return "needs you";
    case "waiting_input": return "needs you";
    case "finished": return "done";
    case "error": return "error";
  }
}

// ---------------------------------------------------------------------------
// Rust bridge

async function respond(requestId: string, decision: UiDecision): Promise<void> {
  await invoke("respond", { requestId, decision });
}

async function resizeToContent(): Promise<void> {
  const height = expanded ? BAR_HEIGHT + PANEL_MAX_HEIGHT : BAR_HEIGHT;
  await getCurrentWindow().setSize(new LogicalSize(WINDOW_WIDTH, height));
}

async function start(): Promise<void> {
  await listen<Snapshot>(STATE_EVENT, (event) => render(event.payload));
  const initial = await invoke<Snapshot>("snapshot");
  render(initial);
}

void start();
