/**
 * Notices (#131): one short phrase per thing that happened, in the color of
 * its meaning. GitHub's come computed from Rust; the agents' come from their
 * sessions. View helpers only.
 */

import { el } from "../shared/dom";
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
}

const GITHUB: Record<GithubNotice["kind"], { tone: ViewNotice["tone"]; say: string }> = {
  critical: { tone: "red", say: "A PR has critical changes, review it!" },
  failed: { tone: "red", say: "A PR failed!" },
  action: { tone: "red", say: "An action failed!" },
  review: { tone: "blue", say: "A PR needs your review" },
  ready: { tone: "green", say: "Your PR is ready" },
};

export function githubNotice(notice: GithubNotice): ViewNotice {
  return { key: `github:${notice.key}`, ...GITHUB[notice.kind], detail: notice.detail, url: notice.url };
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
  text.append(el("b", undefined, notice.say), el("small", undefined, notice.detail));
  return more > 0 ? [text, el("span", "more", `+${more}`)] : [text];
}

/** One row per notice, the most urgent first; a click acts on that one only. */
export function noticeRows(notices: ViewNotice[], act: (notice: ViewNotice) => void): HTMLElement {
  const list = el("div", "notice-rows");
  for (const notice of notices) {
    const row = el("button", "notice-row");
    row.type = "button";
    row.title = notice.url ? `${notice.say} · open on GitHub` : notice.say;
    row.append(el("span", `notice-dot tone-${notice.tone}`), el("b", undefined, notice.say), el("small", undefined, notice.detail));
    row.addEventListener("click", (event) => {
      event.stopPropagation();
      act(notice);
    });
    list.append(row);
  }
  return list;
}
