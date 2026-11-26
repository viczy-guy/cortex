/**
 * Notices (#131): one short phrase per thing that happened, in the color of
 * its meaning. GitHub's come computed from Rust; the agents' come from their
 * sessions. View helpers only.
 */

import { button, el } from "../shared/dom";
import { t } from "../shared/i18n";
import type { GithubNotice, Plan, SessionView } from "../shared/ipc";
import { sessionKey, type Tone } from "./notch";
import { planChips, planSay } from "./plans";

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
  /** An Atlantis run: its line opens its pull request's Atlantis card (#173). */
  plan?: Plan;
  /** The same notice made again, in the language shown now (#154). */
  again: () => ViewNotice;
}

const GITHUB: Record<GithubNotice["kind"], { tone: ViewNotice["tone"]; say: (n: GithubNotice) => string }> = {
  // Atlantis (#161): red when something goes or fails; its own turquoise
  // when it only adds or changes, and when it is applied (#173).
  plan_destroys: { tone: "red", say: (n) => planSay(n.kind, n.plan) },
  plan_failed: { tone: "red", say: (n) => planSay(n.kind, n.plan) },
  apply_failed: { tone: "red", say: (n) => planSay(n.kind, n.plan) },
  plan_ready: { tone: "atlantis", say: (n) => planSay(n.kind, n.plan) },
  applied: { tone: "atlantis", say: (n) => planSay(n.kind, n.plan) },
  failed: { tone: "red", say: () => t("notice.prFailed") },
  action: { tone: "red", say: () => t("notice.actionFailed") },
  review: { tone: "blue", say: () => t("notice.review") },
  // The maintainer's words (#135): "Seu PR#000 (titulo breve do PR) - Foi aprovado!"
  // Blue, like a review request: both are about reviews (#141).
  approved: { tone: "blue", say: (n) => `${t("notice.approvedBefore", n.number)}${n.title}${t("notice.approvedAfter")}` },
};

export function githubNotice(notice: GithubNotice): ViewNotice {
  const { tone, say } = GITHUB[notice.kind];
  const view: ViewNotice = { key: `github:${notice.key}`, tone, say: say(notice), detail: notice.detail, url: notice.url, plan: notice.plan, again: () => githubNotice(notice) };
  // Rust names a run on several projects without an environment in English.
  const projects = notice.plan && !notice.plan.env ? `${notice.plan.projects} projects` : null;
  if (projects && notice.plan && notice.detail.endsWith(projects)) {
    view.detail = notice.detail.slice(0, -projects.length) + t("notice.projects", notice.plan.projects);
  }
  if (notice.kind === "approved") {
    view.around = { before: t("notice.approvedBefore", notice.number), title: notice.title, after: t("notice.approvedAfter") };
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

const AGENT_SAYS: Partial<Record<SessionView["state"], "agent.finished" | "agent.failed" | "agent.needsYou">> = {
  finished: "agent.finished",
  error: "agent.failed",
  waiting_input: "agent.needsYou",
};

const NAMES = new Map([["claude-code", "Claude Code"], ["codex", "Codex"], ["gemini", "Gemini"]]);

/** A session's notice in its current color; none while gray or on a card (the card speaks). */
export function agentNotice(session: SessionView, tone: Tone): ViewNotice | null {
  const say = AGENT_SAYS[session.state];
  if (!say || tone === "gray") return null;
  const folder = session.cwd?.split(/[\\/]/).filter(Boolean).at(-1);
  const name = NAMES.get(session.key.agent) ?? session.key.agent;
  return { key: sessionKey(session), tone, say: t(say), detail: folder ? `${name} · ${folder}` : name, again: () => agentNotice(session, tone)! };
}

const AWAY_COUNTS: [ViewNotice["tone"], "away.red" | "away.amber" | "away.blue" | "away.atlantis" | "away.green"][] = [
  ["red", "away.red"],
  ["amber", "away.amber"],
  ["blue", "away.blue"],
  ["atlantis", "away.atlantis"],
  ["green", "away.green"],
];

/**
 * The pink summary (#157): what came while nobody was looking, one count per
 * color, the most urgent first, and the two ways out.
 */
export function awayLine(notices: ViewNotice[], seeAll: () => void, dismiss: () => void): HTMLElement[] {
  const text = el("span", "say-text");
  text.append(el("b", undefined, t("away.title")));
  const counts = el("span", "away-counts");
  for (const [tone, meaning] of AWAY_COUNTS) {
    const n = notices.filter((notice) => notice.tone === tone).length;
    if (n === 0) continue;
    const count = el("span", "away-count");
    count.setAttribute("role", "img");
    count.setAttribute("aria-label", t(meaning, n));
    count.title = t(meaning, n);
    count.append(el("span", `notice-dot tone-${tone}`), el("span", undefined, String(n)));
    counts.append(count);
  }
  const actions = el("span", "away-buttons");
  actions.append(button("btn-pink", t("away.seeAll"), seeAll), button("btn-neutral", t("away.dismiss"), dismiss));
  return [text, counts, actions];
}

/** The phrase, where it happened, and how many more wait behind it. */
export function sayLine(notice: ViewNotice, more: number): HTMLElement[] {
  const text = el("span", "say-text");
  text.append(phrase(notice), el("small", undefined, notice.detail));
  return more > 0 ? [text, el("span", "more", `+${more}`)] : [text];
}

export interface RowActions {
  /** A click on a plain notice: that one goes, and GitHub's open. */
  act: (notice: ViewNotice) => void;
  clearAll: () => void;
  /** A click on an Atlantis notice: its pull request's card (#173). */
  plan: (notice: ViewNotice) => void;
}

/**
 * One row per notice, the most urgent first; a click acts on that one only,
 * or opens its pull request's Atlantis card (#173). With two or more, "Clear
 * all" on top: in sight on a long list, and never sliding under the pointer
 * as rows go.
 */
export function noticeRows(notices: ViewNotice[], actions: RowActions): HTMLElement {
  const list = el("div", "notice-rows");
  if (notices.length > 1) list.append(button("btn-neutral btn-clear", t("rows.clearAll"), actions.clearAll));
  for (const notice of notices) {
    const row = el("button", "notice-row");
    row.type = "button";
    row.dataset.key = notice.key;
    const plan = notice.plan;
    row.title = plan ? t("rows.showSteps", notice.say) : notice.url ? t("rows.open", notice.say) : notice.say;
    row.append(el("span", `notice-dot tone-${notice.tone}`), phrase(notice), el("small", undefined, notice.detail));
    if (plan) row.append(planChips(plan));
    row.addEventListener("click", (event) => {
      event.stopPropagation();
      if (plan) actions.plan(notice);
      else actions.act(notice);
    });
    list.append(row);
  }
  return list;
}
