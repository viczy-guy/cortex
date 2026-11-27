/**
 * Review requests (#179): one phrase for the ones that came together, and
 * the Reviews tab, every pull request waiting for the user's review. The
 * facts come from GitHub through Rust; only the words and the order are made
 * here.
 */

import { el } from "../shared/dom";
import { t } from "../shared/i18n";
import type { PullRequest } from "../shared/ipc";
import type { ViewNotice } from "./notices";
import { planChips } from "./plans";

/** A review request's notice, as `githubNotice` keys it. */
export const isReview = (notice: ViewNotice): boolean => notice.key.startsWith("github:review:");

/** The newest request first; one GitHub gave no time for goes last. */
const newestFirst = (a: PullRequest, b: PullRequest): number => (b.requested_at ?? "").localeCompare(a.requested_at ?? "") || b.number - a.number;

/** "#1258", "#1258 and #1257", "#1258, #1257 and #1248"; from four on, "#1258, #1257 and 2 more". */
export function listed(numbers: number[]): string {
  const tags = numbers.map((n) => `#${n}`);
  if (tags.length > 3) return t("reviews.andMore", tags.slice(0, 2).join(", "), tags.length - 2);
  return tags.length > 1 ? t("reviews.and", tags.slice(0, -1).join(", "), tags.at(-1) ?? "") : (tags[0] ?? "");
}

/**
 * The review requests that came in one read, as one notice: "3 new reviews ·
 * #1258, #1257 and #1248", or "ana asked for 3 reviews" when one person
 * asked for them all. One alone keeps its own notice.
 */
export function reviewBatch(notices: ViewNotice[], pulls: Map<string, PullRequest>): ViewNotice {
  const [first] = notices;
  if (first && notices.length === 1) return first;
  const known = notices.map((n) => pulls.get(n.url ?? "")).filter((p): p is PullRequest => p !== undefined).sort(newestFirst);
  const authors = new Set(known.map((p) => p.author));
  const [author] = authors;
  const one = authors.size === 1 && known.length === notices.length ? author : null;
  return {
    key: `reviews:${notices.map((n) => n.key).join("|")}`,
    tone: "blue",
    say: one ? t("reviews.byOne", one, notices.length) : t("reviews.new", notices.length),
    detail: listed(known.map((p) => p.number)),
    batch: notices.map((n) => n.key),
    again: () => reviewBatch(notices.map((n) => n.again()), pulls),
  };
}

/** "now", "3 min ago", "4 h ago", "2 days ago": how long a review waits. */
export function waited(iso: string | undefined, now = Date.now()): string {
  const ms = iso ? now - Date.parse(iso) : NaN;
  if (!Number.isFinite(ms)) return "";
  const minutes = Math.floor(Math.max(0, ms) / 60_000);
  if (minutes < 1) return t("ago.now");
  if (minutes < 60) return t("ago.min", minutes);
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? t("ago.h", hours) : t("ago.day", Math.floor(hours / 24));
}

export interface ReviewRow {
  pull: PullRequest;
  /** Its notice's key: a click clears it. */
  key: string;
  /** It came while Cortex was running (or while it was closed), and is not opened yet. */
  fresh: boolean;
}

/** The new ones on top, the newest first; then the one waiting the longest. */
export function reviewOrder(rows: ReviewRow[]): ReviewRow[] {
  const waiting = (a: ReviewRow, b: ReviewRow): number => {
    const [x, y] = [a.pull.requested_at, b.pull.requested_at];
    if (!x || !y) return x ? -1 : y ? 1 : 0;
    return x.localeCompare(y);
  };
  return [...rows].sort((a, b) => Number(b.fresh) - Number(a.fresh) || (a.fresh ? newestFirst(a.pull, b.pull) : waiting(a, b)));
}

/** The Reviews tab: one row per pull request, a click opens it on GitHub. */
export function reviewRows(rows: ReviewRow[], more: number, open: (row: ReviewRow) => void): HTMLElement {
  const list = el("div", "rev-rows");
  if (rows.length === 0) list.append(el("div", "atl-empty", t("reviews.empty")));
  for (const row of rows) {
    const { pull } = row;
    const node = el("button", `rev-row${row.fresh ? " rev-row--new" : ""}`);
    node.type = "button";
    node.title = t("rows.open", `${pull.repo}#${pull.number} ${pull.title}`);
    const sub = el("span", "rev-sub");
    if (row.fresh) sub.append(el("span", "rev-new", t("reviews.newTag")));
    sub.append(el("span", "rev-where", pull.author ? `${pull.repo} · ${pull.author}` : pull.repo));
    if (pull.plan) sub.append(planChips(pull.plan));
    node.append(el("span", "rev-num", `#${pull.number}`), el("span", "rev-title", pull.title), el("span", "rev-age", waited(pull.requested_at)), sub);
    node.addEventListener("click", (event) => {
      event.stopPropagation();
      open(row);
    });
    list.append(node);
  }
  if (more > 0) list.append(el("div", "github-more", t("github.more", more)));
  return list;
}
