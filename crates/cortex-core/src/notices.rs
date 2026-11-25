//! What the island tells the user about GitHub (#131): a review requested, a
//! pull request approved or failing, an Atlantis plan or apply (#161), a
//! failed action. The shell fetches; everything here only reads what came back.

use crate::atlantis::{read_comments, Detail, Plan, Stage};
use crate::github::{Checks, GithubSummary, PullRequest, Review};
use crate::session::truncate;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Pull requests whose comments are read per poll; the others wait for the
/// next. Two keeps the answer (8 comments of up to 64 KiB each, JSON-escaped)
/// under the 2 MiB the shell reads.
pub const MAX_PLAN_FETCH: usize = 2;
/// Plans remembered; the least recently read go first.
const MAX_PLANS: usize = 64;
/// Comments read per pull request: an Atlantis plan, its continuations and
/// the apply after it.
const PLAN_COMMENTS: usize = 8;
/// An apply stays news for an hour: Atlantis merges its pull request at once,
/// which leaves the open lists (#161).
const APPLIED_WINDOW_MS: u64 = 60 * 60 * 1000;
pub const MAX_RUNS: usize = 10;
/// Failed actions older than this are not news.
const RUNS_WINDOW_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_DETAIL_CHARS: usize = 60;
/// A title inside a phrase, its "…" included: the island is at most 460
/// points wide, and the phrase must keep "was approved!" in sight.
const MAX_TITLE_CHARS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    /// Someone asked for your review.
    Review,
    /// Your pull request was approved (#135).
    Approved,
    /// Your pull request's checks fail.
    Failed,
    /// An Atlantis plan of yours is ready and only adds or changes (#161).
    PlanReady,
    /// An Atlantis plan destroys or replaces something.
    PlanDestroys,
    /// An Atlantis plan failed.
    PlanFailed,
    /// An Atlantis apply of yours went through.
    Applied,
    /// An Atlantis apply of yours failed.
    ApplyFailed,
    /// An action you started failed.
    Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
    /// The same while the notice stands; a new failure or plan gets a new one.
    pub key: String,
    pub kind: NoticeKind,
    /// Where it happened: "cortex #128 · checks".
    pub detail: String,
    pub url: String,
    /// The pull request's number and short title, for the phrases that name
    /// it ("Your PR #128 (Short title) was approved!"); 0 and empty otherwise.
    pub number: u64,
    pub title: String,
    /// The Atlantis run it tells about; on a review request, its plan when
    /// that destroys nothing (#161).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
}

/// A failed workflow run the user started, from GitHub's own notice of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedRun {
    pub id: String,
    pub repo: String,
    pub workflow: String,
    pub reference: String,
    pub updated_at: String,
}

impl FailedRun {
    /// GitHub's notice names no run: its repository's Actions page.
    pub fn url(&self) -> String {
        format!("https://github.com/{}/actions", self.repo)
    }
}

/// Everything worth a notice in one account's summary, most urgent first.
pub fn notices(summary: &GithubSummary) -> Vec<Notice> {
    let pr = |kind, key: &str, pull: &PullRequest, what: String| Notice {
        key: format!("{key}:{}", pull.url),
        kind,
        detail: bounded(&format!("{} #{} · {what}", short(&pull.repo), pull.number)),
        url: pull.url.clone(),
        number: pull.number,
        title: short_title(&pull.title),
        plan: None,
    };
    let mut out = Vec::new();
    let recent = iso_utc(summary.fetched_at_ms.saturating_sub(APPLIED_WINDOW_MS));
    let own = summary
        .mine
        .iter()
        .chain(&summary.merged)
        .map(|p| (p, true));
    for (pull, own) in own.chain(summary.review_requests.iter().map(|p| (p, false))) {
        let Some(plan) = &pull.plan else {
            continue;
        };
        let (kind, tag) = match plan.stage {
            Stage::PlanFailed => (NoticeKind::PlanFailed, "plan-failed"),
            Stage::Planned if plan.destroy + plan.replace > 0 => (NoticeKind::PlanDestroys, "plan"),
            Stage::Planned if own && plan.add + plan.change > 0 => (NoticeKind::PlanReady, "plan"),
            Stage::Applied if own && plan.at >= recent => (NoticeKind::Applied, "applied"),
            Stage::ApplyFailed if own => (NoticeKind::ApplyFailed, "apply-failed"),
            _ => continue,
        };
        let where_ = plan.env.clone().unwrap_or_else(|| match plan.projects {
            0 | 1 => "Atlantis".into(),
            n => format!("{n} projects"),
        });
        let mut notice = pr(kind, tag, pull, where_);
        // Another run, another notice.
        notice.key.push_str(&format!(":{}", plan.at));
        notice.plan = Some(plan.clone());
        out.push(notice);
    }
    for pull in &summary.mine {
        // Atlantis tells why its own failures fail the checks.
        let atlantis_failed = pull.plan.as_ref().is_some_and(|p| {
            matches!(
                p.stage,
                Stage::PlanFailed | Stage::Locked | Stage::ApplyFailed
            )
        });
        if pull.checks == Checks::Failing && !atlantis_failed {
            out.push(pr(NoticeKind::Failed, "failed", pull, "checks".into()));
        }
    }
    for run in &summary.failed_runs {
        // On one of the user's own pull requests: "A PR failed!" says it.
        if summary
            .mine
            .iter()
            .any(|p| p.repo == run.repo && p.head == run.reference)
        {
            continue;
        }
        out.push(Notice {
            key: format!("action:{}:{}", run.id, run.updated_at),
            kind: NoticeKind::Action,
            detail: bounded(&format!(
                "{} · {} ({})",
                short(&run.repo),
                run.workflow,
                run.reference
            )),
            url: run.url(),
            number: 0,
            title: String::new(),
            plan: None,
        });
    }
    for pull in &summary.review_requests {
        let who = pull.author.as_deref().unwrap_or("review");
        let mut notice = pr(NoticeKind::Review, "review", pull, who.into());
        // A plan that destroys has its own notice; any other rides along.
        notice.plan = pull
            .plan
            .clone()
            .filter(|p| p.stage == Stage::Planned && p.destroy + p.replace == 0);
        out.push(notice);
    }
    for pull in &summary.mine {
        // Whatever its checks do, or a red-then-green CI would announce the
        // same approval twice; while they fail, "A PR failed!" stands beside it.
        if !pull.draft && pull.review == Review::Approved {
            out.push(Notice {
                key: format!("approved:{}", pull.url),
                kind: NoticeKind::Approved,
                // The phrase names the number already.
                detail: bounded(short(&pull.repo)),
                url: pull.url.clone(),
                number: pull.number,
                title: short_title(&pull.title),
                plan: None,
            });
        }
    }
    out
}

fn short(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}

fn bounded(text: &str) -> String {
    truncate(text, MAX_DETAIL_CHARS)
}

/// "Notices on the island…": at most [`MAX_TITLE_CHARS`] with the "…",
/// never a space before it.
// shortcut: cuts on characters, so a long title can split a joined emoji
// (👩‍💻); cut on graphemes if titles like that turn up.
fn short_title(title: &str) -> String {
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title.into();
    }
    let cut: String = title.chars().take(MAX_TITLE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Notice keys kept between launches, per account, so what came while Cortex
/// was closed is news (#157): at most this many keys in all, each at most this
/// long (a longer one is never news, since it could not be kept).
pub const MAX_KNOWN: usize = 1000;
pub const MAX_KNOWN_KEY: usize = 300;
const MAX_KNOWN_LOGIN: usize = 64;
/// The most the kept keys take as JSON, every character escaped at worst.
pub const MAX_KNOWN_BYTES: u64 = (MAX_KNOWN * (2 * MAX_KNOWN_KEY + 3)
    + crate::github::MAX_ACCOUNTS * (2 * MAX_KNOWN_LOGIN + 6)
    + 2) as u64;

/// Each account's notice keys at its last answer, by lowercase login.
pub type Known = BTreeMap<String, BTreeSet<String>>;

/// What the last run saw, compared once per account: only its first good
/// answer after launch tells what came while Cortex was closed (#157).
#[derive(Debug, Default)]
pub struct LastRun {
    known: Option<Known>,
    compared: BTreeSet<String>,
}

impl LastRun {
    pub fn new(known: Option<Known>) -> Self {
        Self {
            known,
            compared: BTreeSet::new(),
        }
    }

    /// Nothing is news any more: the user changed something (turned GitHub
    /// on, showed an account), which is no coming back.
    pub fn forget(&mut self) {
        self.known = None;
    }

    /// The keys in `login`'s answer the last run did not see; nothing for a
    /// failed answer, after the account's first good one, or for an account
    /// the last run had no answer from.
    pub fn news(&mut self, login: &str, good: bool, notices: &[Notice]) -> Vec<String> {
        let login = login.to_ascii_lowercase();
        let Some(keys) = self.known.as_ref().and_then(|known| known.get(&login)) else {
            return Vec::new();
        };
        if !good || !self.compared.insert(login) {
            return Vec::new();
        }
        notices
            .iter()
            .filter(|n| n.key.len() <= MAX_KNOWN_KEY && !keys.contains(&n.key))
            .map(|n| n.key.clone())
            .collect()
    }
}

/// Each account that answered keeps its keys of now; the others keep theirs,
/// so an account without an answer never loses what it had.
pub fn remember<'a>(
    known: &mut Known,
    answers: impl IntoIterator<Item = (&'a str, &'a GithubSummary)>,
) {
    for (login, summary) in answers {
        let keys = summary.notices.iter().map(|n| n.key.clone()).collect();
        known.insert(login.to_ascii_lowercase(), keys);
    }
}

/// The keys the last run saved; `None` when they cannot be read, and then
/// nothing counts as news.
pub fn known_from_json(text: &str) -> Option<Known> {
    let accounts: BTreeMap<String, Vec<String>> = serde_json::from_str(text).ok()?;
    Some(capped(
        accounts
            .into_iter()
            .map(|(login, keys)| (login, keys.into_iter().collect())),
    ))
}

pub fn known_to_json(known: &Known) -> String {
    let kept = capped(
        known
            .iter()
            .map(|(login, keys)| (login.clone(), keys.clone())),
    );
    serde_json::to_string(&kept).unwrap_or_else(|_| "{}".into())
}

/// The same limits on reading and on writing.
fn capped(accounts: impl Iterator<Item = (String, BTreeSet<String>)>) -> Known {
    let mut left = MAX_KNOWN;
    accounts
        .filter(|(login, _)| !login.is_empty() && login.len() <= MAX_KNOWN_LOGIN)
        .take(crate::github::MAX_ACCOUNTS)
        .map(|(login, keys)| {
            let keys: BTreeSet<String> = keys
                .into_iter()
                .filter(|key| key.len() <= MAX_KNOWN_KEY)
                .take(left)
                .collect();
            left -= keys.len();
            (login.to_ascii_lowercase(), keys)
        })
        .collect()
}

/// "owner/name", as GitHub allows them: owners are letters, digits and
/// hyphens; names may add "_" and "." but are never "." or "..".
pub fn is_repo_name(name: &str) -> bool {
    let Some((owner, repo)) = name.split_once('/') else {
        return false;
    };
    let owner_ok = (1..=39).contains(&owner.len())
        && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let repo_ok = (1..=100).contains(&repo.len())
        && !matches!(repo, "." | "..")
        && repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    owner_ok && repo_ok
}

/// Bounds of GitHub's `X-Poll-Interval`: its usual 60 s, and an hour so a
/// strange value never stops the reads for longer.
const POLL_FLOOR_SECS: u64 = 60;
const POLL_CEILING_SECS: u64 = 60 * 60;

/// A `gh api --include` answer: the seconds GitHub asks to wait before the
/// next read (`X-Poll-Interval`, 60 when absent) and the body. `None` without
/// the blank line that ends the headers.
pub fn split_poll_interval(text: &str) -> Option<(u64, &str)> {
    let mut secs = POLL_FLOOR_SECS;
    let mut rest = text;
    loop {
        let (line, next) = rest.split_once('\n')?;
        rest = next;
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            return Some((secs, rest));
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("x-poll-interval") {
                if let Ok(asked) = value.trim().parse::<u64>() {
                    secs = asked.clamp(POLL_FLOOR_SECS, POLL_CEILING_SECS);
                }
            }
        }
    }
}

/// The REST path for the unread notices of the last day.
pub fn notifications_path(now_ms: u64) -> String {
    format!(
        "notifications?since={}&per_page=50",
        iso_utc(now_ms.saturating_sub(RUNS_WINDOW_MS))
    )
}

/// UTC "YYYY-MM-DDTHH:MM:SSZ" (days to civil date: H. Hinnant's algorithm).
fn iso_utc(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The failed runs among GitHub's notices: "CI workflow run failed for main
/// branch", which GitHub sends to whoever started the run. One per workflow
/// and branch, the latest (GitHub lists the newest first).
pub fn parse_failed_runs(json: &str) -> Vec<FailedRun> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|n| n["reason"] == "ci_activity" && n["unread"] == true)
        .filter(|n| n["subject"]["type"] == "CheckSuite")
        .filter_map(|n| {
            let id = n["id"].as_str().filter(|id| {
                !id.is_empty() && id.len() <= 24 && id.bytes().all(|b| b.is_ascii_digit())
            })?;
            let repo = n["repository"]["full_name"]
                .as_str()
                .filter(|r| is_repo_name(r))?;
            let updated_at = n["updated_at"].as_str().filter(|t| {
                t.len() <= 25
                    && t.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-:.".contains(&b))
            })?;
            let title = n["subject"]["title"].as_str()?;
            let (workflow, rest) = title.split_once(" workflow run failed for ")?;
            let reference = rest
                .strip_suffix(" branch")
                .or_else(|| rest.strip_suffix(" tag"))
                .unwrap_or(rest);
            let clean = |s: &str| -> String {
                s.chars()
                    .filter(|c| !c.is_control())
                    .take(MAX_DETAIL_CHARS)
                    .collect()
            };
            Some(FailedRun {
                id: id.into(),
                repo: repo.into(),
                workflow: clean(workflow.trim()),
                reference: clean(reference.trim()),
                updated_at: updated_at.into(),
            })
        })
        .fold(Vec::<FailedRun>::new(), |mut runs, run| {
            if !runs.iter().any(|r| {
                r.repo == run.repo && r.workflow == run.workflow && r.reference == run.reference
            }) {
                runs.push(run);
            }
            runs
        })
        .into_iter()
        .take(MAX_RUNS)
        .collect()
}

/// One GraphQL request for the latest comments of `count` pull requests,
/// passed as the variables `u0`, `u1`… (never pasted into the query).
pub fn plan_query(count: usize) -> String {
    let vars: Vec<String> = (0..count).map(|i| format!("$u{i}: URI!")).collect();
    let fields: String = (0..count)
        .map(|i| {
            format!(
                " p{i}: resource(url: $u{i}) {{ ... on PullRequest {{ comments(last: {PLAN_COMMENTS}) {{ nodes {{ body url }} }} }} }}"
            )
        })
        .collect();
    format!("query({}) {{{fields} }}", vars.join(", "))
}

/// Each pull request's latest plan and apply comments, in the order asked;
/// `None` for the whole answer when it is not a GraphQL response, and for a
/// pull request the account cannot read or with no plan in sight.
pub fn parse_plans(json: &str, count: usize) -> Option<Vec<Option<Detail>>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let data = v.get("data")?;
    Some(
        (0..count)
            .map(|i| {
                let comments: Vec<(&str, &str)> = data[format!("p{i}")]["comments"]["nodes"]
                    .as_array()?
                    .iter()
                    .filter_map(|c| Some((c["body"].as_str()?, c["url"].as_str().unwrap_or(""))))
                    .collect();
                let detail = read_comments(&comments);
                (detail.plan.is_some() || detail.apply.is_some()).then_some(detail)
            })
            .collect(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanRead {
    /// The pull request's `updatedAt` when its comments were last read.
    updated_at: String,
    /// The plan run they were read for: a detail is only ever shown with it.
    planned_at: String,
    detail: Option<Detail>,
    tried_ms: u64,
}

/// The detail of each pull request's latest plan, per account, read again
/// only once the pull request changed.
#[derive(Debug, Default)]
pub struct PlanCache {
    reads: BTreeMap<String, PlanRead>,
}

impl PlanCache {
    pub const fn new() -> Self {
        Self {
            reads: BTreeMap::new(),
        }
    }

    fn key(login: &str, url: &str) -> String {
        format!("{}\n{url}", login.to_ascii_lowercase())
    }

    /// Up to [`MAX_PLAN_FETCH`] pull requests with an Atlantis run that is
    /// not planning, changed since their comments were read, the least
    /// recently tried first, so a few busy ones never keep the others
    /// waiting: (`url`, `updatedAt`, the plan run).
    pub fn due<'a>(
        &self,
        login: &str,
        pulls: impl IntoIterator<Item = &'a PullRequest>,
    ) -> Vec<(String, String, String)> {
        let mut due: Vec<(u64, &PullRequest, &Plan)> = pulls
            .into_iter()
            .filter_map(|p| {
                let plan = p
                    .plan
                    .as_ref()
                    .filter(|plan| plan.stage != Stage::Planning)?;
                match self.reads.get(&Self::key(login, &p.url)) {
                    Some(read) if read.updated_at == p.updated_at => None,
                    read => Some((read.map_or(0, |r| r.tried_ms), p, plan)),
                }
            })
            .collect();
        due.sort_by_key(|(tried, _, _)| *tried);
        due.into_iter()
            .take(MAX_PLAN_FETCH)
            .map(|(_, p, plan)| (p.url.clone(), p.updated_at.clone(), plan.planned_at.clone()))
            .collect()
    }

    /// The outcome of reading one pull request at `updated_at`, for the plan
    /// run `planned_at`: `None` when the request failed (read again later,
    /// after the others), `Some(None)` when no plan was in sight (later
    /// comments pushed it out: what was known for the same run stands).
    pub fn record(
        &mut self,
        login: &str,
        url: &str,
        updated_at: &str,
        planned_at: &str,
        read: Option<Option<Detail>>,
        now_ms: u64,
    ) {
        let entry = self
            .reads
            .entry(Self::key(login, url))
            .or_insert_with(|| PlanRead {
                updated_at: String::new(),
                planned_at: String::new(),
                detail: None,
                tried_ms: 0,
            });
        entry.tried_ms = now_ms;
        if let Some(detail) = read {
            entry.updated_at = updated_at.into();
            if detail.is_some() || entry.planned_at != planned_at {
                entry.detail = detail;
            }
            entry.planned_at = planned_at.into();
        }
        while self.reads.len() > MAX_PLANS {
            let oldest = self
                .reads
                .iter()
                .min_by_key(|(_, read)| read.tried_ms)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                self.reads.remove(&oldest);
            }
        }
    }

    /// The detail read for this plan run, if any.
    pub fn detail(&self, login: &str, url: &str, planned_at: &str) -> Option<&Detail> {
        self.reads
            .get(&Self::key(login, url))
            .filter(|read| read.planned_at == planned_at)
            .and_then(|read| read.detail.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pull(repo: &str, number: u64) -> PullRequest {
        PullRequest {
            repo: repo.into(),
            number,
            title: "t".into(),
            url: format!("https://github.com/{repo}/pull/{number}"),
            draft: false,
            author: Some("pessoa-b".into()),
            checks: Checks::Pending,
            review: Review::Required,
            updated_at: String::new(),
            plan: None,
            head: format!("feat/{number}"),
        }
    }

    fn plan(stage: Stage, add: u32, destroy: u32) -> Plan {
        Plan {
            stage,
            add,
            change: 0,
            replace: 0,
            destroy,
            projects: 1,
            unchanged: 0,
            env: Some("pre-prod".into()),
            production: false,
            data: vec![],
            drift: false,
            groups: vec![],
            partial: false,
            error: None,
            locked_by: None,
            comment_url: None,
            at: "2026-11-25T10:00:00Z".into(),
            planned_at: "2026-11-25T10:00:00Z".into(),
        }
    }

    fn summary() -> GithubSummary {
        GithubSummary {
            login: "me".into(),
            mine: vec![],
            mine_total: 0,
            review_requests: vec![],
            review_requests_total: 0,
            fetched_at_ms: 0,
            merged: vec![],
            failed_runs: vec![],
            notices: vec![],
        }
    }

    #[test]
    fn each_kind_of_notice_comes_from_its_own_state() {
        let mut s = summary();
        let mut failing = pull("viczy-guy/cortex", 128);
        failing.checks = Checks::Failing;
        failing.review = Review::Approved;
        let mut approved = pull("viczy-guy/cortex", 130);
        approved.review = Review::Approved;
        approved.checks = Checks::Passing;
        approved.title = "Notices on the island, with a long title".into();
        let mut draft = approved.clone();
        draft.number = 131;
        draft.url.push('1');
        draft.draft = true;
        let mut pending = pull("viczy-guy/cortex", 132);
        pending.review = Review::Approved;
        let mut destroys = pull("mt/infra", 482);
        destroys.plan = Some(plan(Stage::Planned, 0, 3));
        s.mine = vec![failing, approved, draft, pending];
        s.review_requests = vec![destroys];
        let run = |reference: &str| FailedRun {
            id: "27485366758".into(),
            repo: "viczy-guy/cortex".into(),
            workflow: "CI".into(),
            reference: reference.into(),
            updated_at: "2026-11-23T16:38:01Z".into(),
        };
        // The second is #128 failing: its own notice says so.
        s.failed_runs = vec![run("main"), run("feat/128")];
        let notices = notices(&s);
        let found: Vec<_> = notices
            .iter()
            .map(|n| (n.kind, n.detail.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                (NoticeKind::PlanDestroys, "infra #482 · pre-prod"),
                (NoticeKind::Failed, "cortex #128 · checks"),
                (NoticeKind::Action, "cortex · CI (main)"),
                (NoticeKind::Review, "infra #482 · pessoa-b"),
                (NoticeKind::Approved, "cortex"),
                (NoticeKind::Approved, "cortex"),
                (NoticeKind::Approved, "cortex"),
            ]
        );
        // Approved and failing: both stand, so passing again announces nothing new.
        assert_eq!(notices[4].number, 128);
        assert_eq!(
            (notices[5].number, notices[5].title.as_str()),
            (130, "Notices on the island,…")
        );
        assert_eq!(
            notices[6].number, 132,
            "checks still running: approved all the same"
        );
        assert_eq!(
            notices[2].url,
            "https://github.com/viczy-guy/cortex/actions"
        );
        assert_eq!(
            notices[0].key,
            "plan:https://github.com/mt/infra/pull/482:2026-11-25T10:00:00Z"
        );
        assert!(
            notices[3].plan.is_none(),
            "the plan that destroys has its own notice"
        );
    }

    #[test]
    fn atlantis_runs_make_their_notices() {
        let now = 1_795_000_000_000;
        let mut s = summary();
        s.fetched_at_ms = now;
        let mut ready = pull("acme/infra", 1);
        ready.plan = Some(plan(Stage::Planned, 3, 0));
        let mut failed = pull("acme/infra", 2);
        failed.plan = Some(plan(Stage::PlanFailed, 0, 0));
        failed.checks = Checks::Failing;
        let mut locked = pull("acme/infra", 3);
        locked.plan = Some(plan(Stage::Locked, 0, 0));
        locked.checks = Checks::Failing;
        let mut planning = pull("acme/infra", 4);
        planning.plan = Some(plan(Stage::Planning, 1, 0));
        let mut apply_failed = pull("acme/infra", 5);
        apply_failed.plan = Some(plan(Stage::ApplyFailed, 1, 0));
        s.mine = vec![ready, failed, locked, planning, apply_failed];
        let mut applied = pull("acme/infra", 6);
        applied.plan = Some(plan(Stage::Applied, 1, 0));
        applied.plan.as_mut().unwrap().at = iso_utc(now - 30 * 60 * 1000);
        let mut long_ago = applied.clone();
        long_ago.url.push('0');
        long_ago.plan.as_mut().unwrap().at = iso_utc(now - 2 * 3600 * 1000);
        s.merged = vec![applied, long_ago];
        let mut review = pull("acme/infra", 7);
        review.plan = Some(plan(Stage::Planned, 2, 0));
        s.review_requests = vec![review];
        let notices = notices(&s);
        let found: Vec<_> = notices.iter().map(|n| (n.kind, n.number)).collect();
        // Planning and a lock wait in the list; Atlantis says why the checks
        // fail; a create-only plan on a review rides on the review's notice.
        assert_eq!(
            found,
            [
                (NoticeKind::PlanReady, 1),
                (NoticeKind::PlanFailed, 2),
                (NoticeKind::ApplyFailed, 5),
                (NoticeKind::Applied, 6),
                (NoticeKind::Review, 7),
            ]
        );
        assert_eq!(notices[4].plan.as_ref().map(|p| p.add), Some(2));
        assert!(notices
            .iter()
            .all(|n| n.kind == NoticeKind::Review || n.plan.is_some()));
    }

    #[test]
    fn a_long_title_is_cut_to_its_limit_without_a_space_before_the_ellipsis() {
        assert_eq!(short_title("Short"), "Short");
        let exact = "a".repeat(MAX_TITLE_CHARS);
        assert_eq!(short_title(&exact), exact);
        let long = short_title(&"é".repeat(40));
        assert_eq!(long.chars().count(), MAX_TITLE_CHARS);
        assert!(long.ends_with('…'));
        assert_eq!(
            short_title("Keep the color until a click, not a hover"),
            "Keep the color until a…"
        );
    }

    #[test]
    fn plans_are_read_again_only_when_the_pull_request_changed_and_never_starve() {
        let mut cache = PlanCache::new();
        let mut pulls: Vec<PullRequest> = (1..=4).map(|n| pull("mt/infra", n)).collect();
        for p in &mut pulls {
            p.updated_at = "t1".into();
            p.plan = Some(plan(Stage::Planned, 1, 0));
        }
        // Still planning: nothing to read yet.
        pulls[3].plan = Some(plan(Stage::Planning, 0, 0));
        let detail = Detail::default();
        let due = cache.due("Me", &pulls);
        assert_eq!(due.len(), MAX_PLAN_FETCH);
        // #1 is read; reading #2 failed.
        cache.record(
            "me",
            &due[0].0,
            "t1",
            &due[0].2,
            Some(Some(detail.clone())),
            10,
        );
        cache.record("me", &due[1].0, "t1", &due[1].2, None, 10);
        let run = "2026-11-25T10:00:00Z";
        assert_eq!(cache.detail("ME", &pulls[0].url, run), Some(&detail));
        assert_eq!(cache.detail("me", &pulls[0].url, "another run"), None);
        // #3 was never tried: it goes before #2, which failed.
        let due: Vec<_> = cache
            .due("me", &pulls)
            .into_iter()
            .map(|(u, _, _)| u)
            .collect();
        assert_eq!(due, [pulls[2].url.clone(), pulls[1].url.clone()]);
        // A change brings #1 back; a window without the plan keeps what was
        // known for the same run, not for another.
        pulls[0].updated_at = "t2".into();
        assert!(cache
            .due("me", &pulls)
            .iter()
            .any(|(u, _, _)| *u == pulls[0].url));
        cache.record("me", &pulls[0].url, "t2", run, Some(None), 20);
        assert_eq!(cache.detail("me", &pulls[0].url, run), Some(&detail));
        cache.record("me", &pulls[0].url, "t3", "a new run", Some(None), 30);
        assert_eq!(cache.detail("me", &pulls[0].url, "a new run"), None);
        // Another account reads for itself.
        assert_eq!(cache.detail("other", &pulls[0].url, run), None);
        assert_eq!(cache.due("other", &pulls).len(), MAX_PLAN_FETCH);
    }

    #[test]
    fn the_plan_cache_forgets_the_least_recently_read_first() {
        let mut cache = PlanCache::new();
        for i in 0..=MAX_PLANS as u64 {
            cache.record(
                "me",
                &format!("u{i}"),
                "t",
                "r",
                Some(Some(Detail::default())),
                i + 1,
            );
        }
        assert_eq!(cache.reads.len(), MAX_PLANS);
        assert_eq!(cache.detail("me", "u0", "r"), None);
        assert!(cache.detail("me", &format!("u{MAX_PLANS}"), "r").is_some());
    }

    #[test]
    fn plan_comments_are_asked_through_variables_and_read_back_in_order() {
        assert_eq!(
            plan_query(2),
            "query($u0: URI!, $u1: URI!) { p0: resource(url: $u0) { ... on PullRequest { comments(last: 8) { nodes { body url } } } } p1: resource(url: $u1) { ... on PullRequest { comments(last: 8) { nodes { body url } } } } }"
        );
        let response = json!({"data": {
            "p0": {"comments": {"nodes": [{"body": "Ran Plan for dir: `a` workspace: `default`\n```\n  # aws_s3_bucket.logs will be destroyed\n```", "url": "https://github.com/o/r/pull/1#c1"}]}},
            "p1": null,
            "p2": {"comments": {"nodes": [{"body": "lgtm", "url": "https://github.com/o/r/pull/2#c1"}]}}
        }});
        let read = parse_plans(&response.to_string(), 3).unwrap();
        let plan = read[0].as_ref().unwrap().plan.as_ref().unwrap();
        assert_eq!(plan.url, "https://github.com/o/r/pull/1#c1");
        assert_eq!(plan.projects[0].changes.len(), 1);
        assert_eq!((read[1].as_ref(), read[2].as_ref()), (None, None));
        assert_eq!(parse_plans("{\"errors\":[]}", 1), None);
    }

    #[test]
    fn only_unread_failed_runs_count_and_odd_entries_are_dropped() {
        let item = |reason: &str, unread: bool, title: &str, repo: &str| {
            json!({"id": "27485366758", "reason": reason, "unread": unread, "updated_at": "2026-11-23T16:38:01Z",
                   "subject": {"type": "CheckSuite", "title": title, "url": null},
                   "repository": {"full_name": repo}})
        };
        let list = json!([
            item(
                "ci_activity",
                true,
                "CI workflow run failed for main branch",
                "viczy-guy/cortex"
            ),
            item(
                "ci_activity",
                true,
                "Release workflow run failed for v0.3.2 tag",
                "viczy-guy/cortex"
            ),
            // An older failure of the same workflow on the same branch: one notice.
            item(
                "ci_activity",
                true,
                "CI workflow run failed for main branch",
                "viczy-guy/cortex"
            ),
            item(
                "ci_activity",
                false,
                "CI workflow run failed for main branch",
                "viczy-guy/cortex"
            ),
            item(
                "review_requested",
                true,
                "CI workflow run failed for main branch",
                "viczy-guy/cortex"
            ),
            item(
                "ci_activity",
                true,
                "CI workflow run succeeded for main branch",
                "viczy-guy/cortex"
            ),
            item(
                "ci_activity",
                true,
                "CI workflow run failed for main branch",
                "../etc"
            ),
        ]);
        let runs = parse_failed_runs(&list.to_string());
        let found: Vec<_> = runs
            .iter()
            .map(|r| (r.workflow.as_str(), r.reference.as_str()))
            .collect();
        assert_eq!(found, [("CI", "main"), ("Release", "v0.3.2")]);
        assert!(parse_failed_runs("{}").is_empty());
    }

    #[test]
    fn the_notices_window_starts_a_day_before_now() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_795_451_881_000), "2026-11-23T16:38:01Z");
        assert_eq!(iso_utc(1_709_251_199_000), "2024-02-29T23:59:59Z");
        assert_eq!(iso_utc(951_868_800_000), "2000-03-01T00:00:00Z");
        assert_eq!(
            notifications_path(1_795_451_881_000),
            "notifications?since=2026-11-22T16:38:01Z&per_page=50"
        );
    }

    #[test]
    fn the_wait_github_asks_for_comes_with_the_body() {
        let answer = "HTTP/2.0 200 OK\nContent-Type: application/json\r\nx-poll-interval: 120\r\n\r\n[{\"id\":\"1\"}]";
        assert_eq!(split_poll_interval(answer), Some((120, "[{\"id\":\"1\"}]")));
        // Absent, too short, too long or not a number: within the bounds.
        assert_eq!(
            split_poll_interval("HTTP/2.0 200 OK\n\n[]"),
            Some((60, "[]"))
        );
        assert_eq!(
            split_poll_interval("X-Poll-Interval: 5\n\n[]"),
            Some((60, "[]"))
        );
        assert_eq!(
            split_poll_interval("X-Poll-Interval: 999999\n\n[]"),
            Some((3600, "[]"))
        );
        assert_eq!(
            split_poll_interval("X-Poll-Interval: soon\n\n[]"),
            Some((60, "[]"))
        );
        // Headers that never end: not an answer.
        assert_eq!(
            split_poll_interval("HTTP/2.0 200 OK\nX-Poll-Interval: 60"),
            None
        );
        assert_eq!(split_poll_interval("[]"), None);
    }

    #[test]
    fn repository_names_are_owner_and_name_only() {
        assert!(is_repo_name("viczy-guy/cortex"));
        assert!(is_repo_name("mt/infra.tf_v2"));
        for bad in [
            "cortex", "a/b/c", "../etc", "a/..", "a/", "/b", "a b/c", "a/b?x=1",
        ] {
            assert!(!is_repo_name(bad), "{bad}");
        }
    }

    fn keyed(key: &str) -> Notice {
        Notice {
            key: key.into(),
            kind: NoticeKind::Review,
            detail: String::new(),
            url: String::new(),
            number: 0,
            title: String::new(),
            plan: None,
        }
    }

    fn answered(keys: &[&str]) -> GithubSummary {
        GithubSummary {
            login: "ana".into(),
            mine: Vec::new(),
            mine_total: 0,
            review_requests: Vec::new(),
            review_requests_total: 0,
            fetched_at_ms: 0,
            merged: Vec::new(),
            failed_runs: Vec::new(),
            notices: keys.iter().map(|k| keyed(k)).collect(),
        }
    }

    #[test]
    fn only_each_accounts_first_good_answer_names_what_came_while_closed() {
        let mut known = Known::new();
        remember(
            &mut known,
            [("Ana", &answered(&["review:a", "action:1:t"]))],
        );
        let mut run = LastRun::new(Some(known.clone()));
        let long = format!("review:{}", "x".repeat(MAX_KNOWN_KEY));
        let now = [keyed("review:a"), keyed("approved:b"), keyed(&long)];
        // A failed answer is no answer: the account is compared later.
        assert!(run.news("ana", false, &now).is_empty());
        assert_eq!(run.news("ANA", true, &now), ["approved:b"]);
        assert!(run.news("ana", true, &[keyed("failed:c")]).is_empty());
        // An account the last run never heard from: its first answer is not news.
        assert!(run.news("Bo", true, &now).is_empty());
        let mut run = LastRun::new(Some(known.clone()));
        run.forget();
        assert!(run.news("ana", true, &now).is_empty());
        assert!(LastRun::new(None).news("ana", true, &now).is_empty());
        assert_eq!(known_from_json(&known_to_json(&known)), Some(known));
    }

    #[test]
    fn an_account_without_an_answer_keeps_what_it_had() {
        let mut known = Known::new();
        remember(
            &mut known,
            [
                ("ana", &answered(&["review:a"])),
                ("bo", &answered(&["failed:b"])),
            ],
        );
        // Only Ana answered this time.
        remember(&mut known, [("Ana", &answered(&["approved:c"]))]);
        assert_eq!(known["ana"], ["approved:c".to_owned()].into());
        assert_eq!(known["bo"], ["failed:b".to_owned()].into());
    }

    #[test]
    fn saved_keys_are_capped_and_anything_else_reads_as_none() {
        let long = "x".repeat(MAX_KNOWN_KEY + 1);
        let many: Vec<String> = (0..MAX_KNOWN + 5).map(|i| format!("k{i}")).collect();
        let text = serde_json::json!({ "ana": many, "bo": ["b"] }).to_string();
        let read = known_from_json(&text).unwrap();
        assert_eq!(read["ana"].len(), MAX_KNOWN);
        assert!(read["bo"].is_empty());
        // The most that can be written can be read back.
        let widest: Known = (0..crate::github::MAX_ACCOUNTS)
            .map(|a| {
                let login = format!("{a}{}", "\"".repeat(MAX_KNOWN_LOGIN - 2));
                let keys = (0..MAX_KNOWN / crate::github::MAX_ACCOUNTS)
                    .map(|i| format!("{a}{i:04}{}", "\"".repeat(MAX_KNOWN_KEY - 6)))
                    .collect();
                (login, keys)
            })
            .collect();
        assert!(known_to_json(&widest).len() as u64 <= MAX_KNOWN_BYTES);
        let text = serde_json::json!({ "ana": ["ok", long] }).to_string();
        assert_eq!(
            known_from_json(&text).unwrap()["ana"],
            ["ok".to_owned()].into()
        );
        for bad in ["", "[]", "{\"a\": [1]}", "null", "{\"a\": [\"b\"]"] {
            assert_eq!(known_from_json(bad), None, "{bad}");
        }
    }
}
