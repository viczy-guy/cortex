/**
 * Atlantis, live (#173): the sign on the island (Atlantis's round icon, a
 * ring turning around it while runs go, and how many) and the Atlantis tab,
 * one card per pull request with its five steps. The facts come from
 * `cortex_core`; only the words and the clock are made here.
 */

import { button, el } from "../shared/dom";
import { t } from "../shared/i18n";
import type { AtlantisRun, Plan, Watched } from "../shared/ipc";
import { atlantisIcon } from "./notch";
import { planChips, planSummary } from "./plans";

/** A plan or an apply going now. */
export const running = (plan: Plan | null): boolean => plan?.stage === "planning" || plan?.stage === "applying";

/** The list's two tabs, once Atlantis has something to show. */
export type Tab = "notices" | "atlantis";

/** A small ring turning, for a run going. */
const mini = (): HTMLElement => el("span", "atl-mini");

/** A button of the island's own look; a click on it is not a click on the island. */
function plain(className: string, onClick: () => void): HTMLButtonElement {
  const node = el("button", className);
  node.type = "button";
  node.addEventListener("click", (event) => {
    event.stopPropagation();
    onClick();
  });
  return node;
}

/** The sign: Atlantis's icon in its turning ring, and how many runs go. */
export function atlantisSign(count: number, open: () => void): HTMLButtonElement {
  const sign = plain("atl-sign", open);
  sign.title = t("atl.sign", count);
  const ring = el("span", "atl-ring");
  ring.append(atlantisIcon());
  sign.append(ring, el("span", undefined, String(count)));
  return sign;
}

/** "Notices" and "Atlantis", each with its count. */
export function atlantisTabs(shown: Tab, notices: number, runs: number, pick: (tab: Tab) => void): HTMLElement {
  const tabs = el("div", "atl-tabs");
  tabs.setAttribute("role", "tablist");
  const tab = (id: Tab, label: string, extra: HTMLElement[]): HTMLButtonElement => {
    const node = plain("atl-tab", () => pick(id));
    node.setAttribute("role", "tab");
    node.setAttribute("aria-selected", String(shown === id));
    node.append(el("span", undefined, label), ...extra);
    return node;
  };
  tabs.append(
    tab("notices", t("atl.tab.notices"), notices > 0 ? [el("span", "atl-n", String(notices))] : []),
    tab("atlantis", t("atl.tab.atlantis"), runs > 0 ? [mini(), el("span", "atl-n", String(runs))] : []),
  );
  return tabs;
}

/** "1:05", "1:02:03": how long since an ISO time. */
export function since(iso: string, now = Date.now()): string {
  const ms = now - Date.parse(iso);
  const total = Number.isFinite(ms) ? Math.max(0, Math.floor(ms / 1000)) : 0;
  const pad = (n: number): string => String(n).padStart(2, "0");
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(total % 60)}` : `${minutes}:${pad(total % 60)}`;
}

function timer(iso: string): HTMLElement {
  const node = el("span", "atl-timer", since(iso));
  node.dataset.since = iso;
  return node;
}

/** Every clock on the island, a second later: only text changes, no redraw. */
export function tick(root: HTMLElement): void {
  for (const node of root.querySelectorAll<HTMLElement>("[data-since]")) node.textContent = since(node.dataset.since ?? "");
}

type StepState = "todo" | "run" | "done" | "wait" | "lock" | "fail";
interface Step {
  state: StepState;
  sub?: string;
  since?: string;
}

const STEPS = ["atl.step.prepare", "atl.step.plan", "atl.step.approval", "atl.step.apply", "atl.step.merge"] as const;

/** "+3 ~1": what the plan does, or that it changes nothing. */
function counts(plan: Plan): string {
  const parts = [
    plan.add ? `+${plan.add}` : "",
    plan.change ? `~${plan.change}` : "",
    plan.replace ? `±${plan.replace}` : "",
    plan.destroy ? `−${plan.destroy}` : "",
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" ") : t("atl.noChanges");
}

/** Preparar → Plan → Aprovação → Apply → Merge, as the run stands. */
function steps(w: Watched, plan: Plan): Step[] {
  const { stage, live } = plan;
  const done = plan.runs.filter((r) => r.state === "done").length;
  const approved = w.pull.review === "approved";
  const prepare: Step = stage === "planning" && live?.preparing ? { state: "run", since: live.since } : { state: "done" };
  const planStep: Step =
    stage === "planning"
      ? live?.preparing ? { state: "todo" } : { state: "run", sub: t("atl.ready", done) }
      : stage === "plan_failed"
        ? { state: "fail", sub: t("atl.failed") }
        : stage === "locked"
          ? { state: "lock", sub: plan.locked_by ? t("atl.by", plan.locked_by) : t("atl.locked") }
          : { state: "done", sub: counts(plan) };
  const applying = stage === "applying" || stage === "applied" || stage === "apply_failed";
  const approval: Step = applying
    ? { state: "done", sub: w.own ? t("atl.approved") : undefined }
    : stage !== "planned"
      ? { state: "todo" }
      : !w.own
        ? { state: "wait", sub: t("atl.yourReview") }
        : approved ? { state: "done", sub: t("atl.approved") } : { state: "wait", sub: t("atl.waiting") };
  const total = plan.runs.filter((r) => r.state !== "skipped").length;
  const apply: Step =
    stage === "applying"
      ? live?.preparing ? { state: "run", sub: t("atl.preparing"), since: live.since } : { state: "run", sub: t("atl.of", done, total) }
      : stage === "applied"
        ? { state: "done", sub: t("atl.applied") }
        : stage === "apply_failed"
          ? { state: "fail", sub: t("atl.failed") }
          : stage === "planned" && w.own && approved ? { state: "wait", sub: t("atl.yourTurn") } : { state: "todo" };
  const merge: Step = w.merged ? { state: "done", sub: t("atl.merged") } : stage === "applied" ? { state: "wait" } : { state: "todo" };
  return [prepare, planStep, approval, apply, merge];
}

function stepList(list: Step[]): HTMLElement {
  const node = el("ol", "atl-steps");
  STEPS.forEach((key, i) => {
    const step = list[i] ?? { state: "todo" };
    const item = el("li", `atl-${step.state}`);
    item.append(el("i", "atl-node"), el("span", undefined, t(key)));
    if (step.sub || step.since) {
      const small = el("small", undefined, step.sub && step.since ? `${step.sub} ` : (step.sub ?? ""));
      if (step.since) small.append(timer(step.since));
      item.append(small);
    }
    node.append(item);
  });
  return node;
}

function logLink(url: string, live: boolean, open: (url: string) => void): HTMLButtonElement {
  const node = plain("atl-link", () => open(url));
  node.textContent = t(live ? "atl.liveLog" : "atl.log");
  return node;
}

/** What happens now, in a line: what runs and for how long, or what it waits for. */
function nowLine(w: Watched, plan: Plan, open: (url: string) => void): HTMLElement {
  const node = el("div", "atl-now");
  const say = (text: string): void => void node.append(el("span", undefined, text));
  const small = (text: string | null): void => void (text && node.append(el("small", undefined, text)));
  const live = plan.live;
  switch (plan.stage) {
    case "planning":
    case "applying": {
      const applying = plan.stage === "applying";
      if (live?.preparing) say(t(applying ? "atl.now.prepareApply" : "atl.now.prepare"));
      else say(t(applying ? "atl.now.applying" : "atl.now.planning"));
      if (live?.project) node.append(el("code", undefined, plan.runs.find((r) => r.name === live.project)?.label ?? live.project));
      if (live) node.append(timer(live.since));
      if (live?.url) node.append(logLink(live.url, true, open));
      if (live?.preparing && !applying) small(t("atl.now.beforeFirst"));
      break;
    }
    case "locked":
      say(plan.locked_by ? t("run.lockedBy", plan.locked_by) : t("run.locked"));
      small(plan.locked_by ? t("plan.waitsFor", plan.locked_by) : t("plan.waitsOther"));
      break;
    case "plan_failed":
      say(t("plan.failed"));
      small(plan.error ?? t("plan.failedReason"));
      break;
    case "apply_failed":
      say(t("plan.applyFailed"));
      small(plan.error ?? t("plan.applyFailedReason"));
      break;
    case "planned":
      if (!w.own) say(t("atl.now.yourReview"));
      else if (w.pull.review !== "approved") say(t("atl.now.approval"));
      else node.append(el("span", undefined, t("atl.now.comment")), el("code", undefined, "atlantis apply"));
      break;
    case "applied":
      say(t(w.merged ? "atl.now.merged" : "atl.now.applied"));
      break;
  }
  return node;
}

const MARK: Record<AtlantisRun["state"], string> = { running: "", failed: "✕", queued: "·", done: "✓", skipped: "–" };

/** The projects of the run shown, each with its log. */
function projectList(plan: Plan, open: (url: string) => void): HTMLElement | null {
  if (plan.runs.length === 0) return null;
  // Several accounts: each project says which.
  const many = plan.envs.length > 1;
  const list = el("ul", "atl-projs");
  list.dataset.scroll = "projects";
  for (const run of plan.runs) {
    const item = el("li", run.state === "skipped" ? "atl-skip" : undefined);
    const mark = el("span", "atl-ps", MARK[run.state]);
    if (run.state === "running") mark.append(mini());
    const name = el("span", "atl-pn", many && run.env ? `${run.env} · ${run.label}` : run.label);
    name.title = run.name;
    const right = el("span", "atl-right");
    if (run.state === "running" && plan.live?.project === run.name) right.append(timer(plan.live.since));
    else if (run.state === "queued") right.append(el("span", undefined, t("atl.queued")));
    else if (run.state === "failed") right.append(el("span", undefined, t("atl.failed")));
    else if (run.state === "done" && run.add + run.change + run.destroy > 0) right.append(planChips({ ...run, replace: 0 }));
    else if (run.state !== "running") right.append(el("span", undefined, t("atl.noChanges")));
    if (run.url) right.append(logLink(run.url, run.state === "running", open));
    item.append(mark, name, right);
    list.append(item);
  }
  if (plan.runs_hidden > 0) list.append(el("li", "atl-skip", t("atl.more", plan.runs_hidden)));
  return list;
}

export interface CardActions {
  /** A link: the pull request, its plan's comment, a live log. */
  open: (url: string) => void;
  /** The card's head: it opens, or closes. */
  toggle: (url: string) => void;
}

/** A pull request in the Atlantis tab: the steps always; open, what runs now, the plan, the projects and the links. */
export function atlantisCard(w: Watched, opened: boolean, actions: CardActions): HTMLElement {
  const card = el("div", "atl-card");
  const plan = w.pull.plan;
  if (!plan) return card;
  card.dataset.url = w.pull.url;
  const head = plain("atl-head", () => actions.toggle(w.pull.url));
  head.setAttribute("aria-expanded", String(opened));
  const mark = el("span", "atl-state");
  if (running(plan)) mark.append(mini());
  else mark.textContent = w.merged ? "✓" : plan.stage === "locked" ? "‖" : "●";
  const title = el("span", "atl-title");
  const repo = w.pull.repo.split("/").at(-1) ?? w.pull.repo;
  title.append(el("b", undefined, `#${w.pull.number} · ${w.pull.title}`), el("small", undefined, `${repo} · ${t(w.own ? "atl.yours" : "atl.theirs")}`));
  // One tag per account the pull request touches; production in red.
  const envs = el("span", "atl-envs");
  for (const env of plan.envs) envs.append(el("span", `plan-env${env.production ? " plan-env--prod" : ""}`, env.name));
  head.append(mark, title, envs);
  card.append(head, stepList(steps(w, plan)));
  if (!opened) return card;
  card.append(nowLine(w, plan, actions.open));
  if (plan.stage === "planned" || plan.stage === "applying" || plan.stage === "applied") card.append(...planSummary(plan));
  const projects = projectList(plan, actions.open);
  if (projects) card.append(projects);
  const links = el("div", "atl-acts");
  links.append(button("btn-neutral", t("plan.openPr"), () => actions.open(w.pull.url)));
  const comment = plan.comment_url;
  if (comment) links.append(button("btn-neutral", t("plan.full"), () => actions.open(comment)));
  card.append(links);
  return card;
}
