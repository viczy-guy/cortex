/**
 * Notices (#131): one short phrase per thing that happened, in the color of
 * its meaning. GitHub's come computed from Rust; the agents' come from their
 * sessions. View helpers only.
 */

import { button, el } from "../shared/dom";
import type { GithubNotice, SessionView } from "../shared/ipc";
import { sessionKey, type Tone } from "./notch";

export interface ViewNotice {
  key: string;
  tone: Exclude<Tone, "gray">;
  say: string;
  /** Where: "cortex #128 · checks", "Claude Code · cortex". */
  detail: string;
  /** GitHub's: opened on a click. */
  url?: string;
  /** A phrase around a title: only the title gives way when space runs out. */
  around?: { before: string; title: string; after: string };
}

const GITHUB: Record<GithubNotice["kind"], { tone: ViewNotice["tone"]; say: (n: GithubNotice) => string }> = {
  critical: { tone: "red", say: () => "A PR has critical changes, review it!" },
  failed: { tone: "red", say: () => "A PR failed!" },
  action: { tone: "red", say: () => "An action failed!" },
  review: { tone: "blue", say: () => "A PR needs your review" },
  // The maintainer's words (#135): "Seu PR#000 (titulo breve do PR) - Foi aprovado!"
  // Blue, like a review request: both are about reviews (#141).
  approved: { tone: "blue", say: (n) => `Your PR #${n.number} (${n.title}) was approved!` },
};

export function githubNotice(notice: GithubNotice): ViewNotice {
  const { tone, say } = GITHUB[notice.kind];
  const view: ViewNotice = { key: `github:${notice.key}`, tone, say: say(notice), detail: notice.detail, url: notice.url };
  if (notice.kind === "approved") {
    view.around = { before: `Your PR #${notice.number} (`, title: notice.title, after: ") was approved!" };
  }
  return view;
}

/** The phrase in bold; around a title, "was approved!" stays in sight. */
function phrase(notice: ViewNotice): HTMLElement {
  if (!notice.around) return el("b", undefined, notice.say);
  const { before, title, after } = notice.around;
  const node = el("b", "phrase");
  node.title = notice.say;
  node.append(el("span", undefined, before), el("span", "phrase-title", title), el("span", undefined, after));
  return node;
}

const AGENT_SAYS: Partial<Record<SessionView["state"], string>> = {
  finished: "An agent finished",
  error: "An agent failed",
  waiting_input: "An agent needs you",
};

const NAMES = new Map([["claude-code", "Claude Code"], ["codex", "Codex"], ["gemini", "Gemini"]]);

/** A session's notice in its current color; none while gray or on a card (the card speaks). */
export function agentNotice(session: SessionView, tone: Tone): ViewNotice | null {
  const say = AGENT_SAYS[session.state];
  if (!say || tone === "gray") return null;
  const folder = session.cwd?.split(/[\\/]/).filter(Boolean).at(-1);
  const name = NAMES.get(session.key.agent) ?? session.key.agent;
  return { key: sessionKey(session), tone, say, detail: folder ? `${name} · ${folder}` : name };
}

/** The phrase, where it happened, and how many more wait behind it. */
export function sayLine(notice: ViewNotice, more: number): HTMLElement[] {
  const text = el("span", "say-text");
  text.append(phrase(notice), el("small", undefined, notice.detail));
  return more > 0 ? [text, el("span", "more", `+${more}`)] : [text];
}

/**
 * One row per notice, the most urgent first; a click acts on that one only.
 * With two or more, "Clear all" on top: in sight on a long list, and never
 * sliding under the pointer as rows go.
 */
export function noticeRows(notices: ViewNotice[], act: (notice: ViewNotice) => void, clearAll: () => void): HTMLElement {
  const list = el("div", "notice-rows");
  if (notices.length > 1) list.append(button("btn-neutral btn-clear", "Clear all", clearAll));
  for (const notice of notices) {
    const row = el("button", "notice-row");
    row.type = "button";
    row.title = notice.url ? `${notice.say} · open on GitHub` : notice.say;
    row.append(el("span", `notice-dot tone-${notice.tone}`), phrase(notice), el("small", undefined, notice.detail));
    row.addEventListener("click", (event) => {
      event.stopPropagation();
      act(notice);
    });
    list.append(row);
  }
  return list;
}
