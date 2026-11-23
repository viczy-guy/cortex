import { button, el } from "../shared/dom";
import { api, type AccountStatus, type GithubSummary, type PullRequest } from "../shared/ipc";

type SummarizedAccount = AccountStatus & { summary: GithubSummary };

export function shownAccounts(accounts: AccountStatus[]): SummarizedAccount[] {
  return accounts.filter((account): account is SummarizedAccount => account.shown && account.summary !== null);
}

export function githubCard(accounts: SummarizedAccount[]): HTMLElement {
  const card = el("div", "card github-card");
  const head = el("div", "card-head");
  head.append(el("span", "title", "GitHub"));
  const [first, second] = accounts;
  if (first && !second) head.append(el("span", "meta", first.login));
  card.append(head);
  const error = el("div", "note note-error");
  function section(group: HTMLElement, title: string, pulls: PullRequest[], total: number, own: boolean): void {
    if (pulls.length === 0 && total === 0) return;
    group.append(el("div", "github-section", title));
    for (const pr of pulls) {
      const row = button("github-row", "", () => {
        void api.openUrl(pr.url).catch((reason: unknown) => { error.textContent = String(reason); card.append(error); });
      });
      row.title = `${pr.repo}#${pr.number} ${pr.title}`;
      row.append(el("span", `state-dot github-dot-${pr.checks}`), el("span", "github-repo", `${pr.repo}#${pr.number}`), el("span", "github-title", pr.title));
      if (pr.draft) row.append(el("span", "github-tag", "draft"));
      if (own && (pr.review === "approved" || pr.review === "changes_requested")) row.append(el("span", "github-tag", pr.review === "approved" ? "approved" : "changes requested"));
      group.append(row);
    }
    if (total > pulls.length) group.append(el("div", "github-more", `+${total - pulls.length} more`));
  }
  for (const { login, summary } of accounts) {
    const group = second ? el("div", "github-group") : card;
    if (second) {
      group.append(el("div", "github-account", login));
      card.append(group);
    }
    section(group, "Review requested", summary.review_requests, summary.review_requests_total, false);
    section(group, "Your pull requests", summary.mine, summary.mine_total, true);
    if (summary.mine_total === 0 && summary.review_requests_total === 0 && summary.mine.length === 0 && summary.review_requests.length === 0) group.append(el("div", "github-more", "Nothing open."));
  }
  return card;
}
