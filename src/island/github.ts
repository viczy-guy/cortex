import { button, el } from "../shared/dom";
import { api, type GithubSummary, type PullRequest } from "../shared/ipc";

export function githubPill(summary: GithubSummary, toggle?: () => void): HTMLElement {
  const failing = summary.mine.filter((pr) => pr.checks === "failing").length;
  const labels: string[] = [];
  if (summary.review_requests_total > 0) labels.push(`${summary.review_requests_total} reviews`);
  if (failing > 0) labels.push(`${failing} failing`);
  if (labels.length === 0) labels.push(summary.mine_total > 0 ? `${summary.mine_total} open PRs` : "no open PRs");
  const node = toggle ? button("pill github-pill", "", toggle) : el("div", "pill github-pill");
  node.append(el("span", `state-dot github-dot-${failing > 0 ? "failing" : summary.review_requests_total > 0 ? "attention" : "none"}`), el("span", "ticker", labels.join(" · ")));
  return node;
}

export function githubCard(summary: GithubSummary): HTMLElement {
  const card = el("div", "card github-card");
  const head = el("div", "card-head");
  head.append(el("span", "title", "GitHub"), el("span", "meta", summary.login));
  card.append(head);
  const error = el("div", "note note-error");
  function section(title: string, pulls: PullRequest[], total: number, own: boolean): void {
    if (pulls.length === 0 && total === 0) return;
    card.append(el("div", "github-section", title));
    for (const pr of pulls) {
      const row = button("github-row", "", () => {
        void api.openUrl(pr.url).catch((reason: unknown) => { error.textContent = String(reason); card.append(error); });
      });
      row.title = `${pr.repo}#${pr.number} ${pr.title}`;
      row.append(el("span", `state-dot github-dot-${pr.checks}`), el("span", "github-repo", `${pr.repo}#${pr.number}`), el("span", "github-title", pr.title));
      if (pr.draft) row.append(el("span", "github-tag", "draft"));
      if (own && (pr.review === "approved" || pr.review === "changes_requested")) row.append(el("span", "github-tag", pr.review === "approved" ? "approved" : "changes requested"));
      card.append(row);
    }
    if (total > pulls.length) card.append(el("div", "github-more", `+${total - pulls.length} more`));
  }
  section("Review requested", summary.review_requests, summary.review_requests_total, false);
  section("Your pull requests", summary.mine, summary.mine_total, true);
  if (summary.mine_total === 0 && summary.review_requests_total === 0 && summary.mine.length === 0 && summary.review_requests.length === 0) card.append(el("div", "github-more", "Nothing open."));
  return card;
}
