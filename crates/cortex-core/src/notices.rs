//! What the island tells the user about GitHub (#131): a review requested, a
//! pull request approved or failing, a plan that destroys something, a failed
//! action. The shell fetches; everything here only reads what came back.

use crate::github::{Checks, GithubSummary, PullRequest, Review};
use crate::session::truncate;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Pull requests whose comments are read per poll; the others wait for the
/// next. Two keeps the answer (5 comments of up to 64 KiB each, JSON-escaped)
/// well under the 2 MiB the shell reads.
pub const MAX_PLAN_FETCH: usize = 2;
/// Plans remembered; the least recently read go first.
const MAX_PLANS: usize = 256;
/// Comments read per pull request: an Atlantis plan and its continuations.
const PLAN_COMMENTS: usize = 5;
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
    /// An Atlantis plan on the pull request destroys something.
    Critical,
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
    };
    let mut out = Vec::new();
    for pull in summary.mine.iter().chain(&summary.review_requests) {
        if pull.destroy > 0 {
            let mut notice = pr(
                NoticeKind::Critical,
                "critical",
                pull,
                format!("{} to destroy", pull.destroy),
            );
            // Another plan, another notice.
            notice.key.push_str(&format!(":{}", pull.destroy));
            out.push(notice);
        }
    }
    for pull in &summary.mine {
        if pull.checks == Checks::Failing {
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
        });
    }
    for pull in &summary.review_requests {
        let who = pull.author.as_deref().unwrap_or("review");
        out.push(pr(NoticeKind::Review, "review", pull, who.into()));
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

/// Notice keys kept between launches, so what came while Cortex was closed is
/// news (#157): at most this many, each at most this long.
pub const MAX_KNOWN: usize = 1000;
const MAX_KNOWN_KEY: usize = 512;

/// The keys of `notices` that were not there when Cortex last ran.
pub fn new_since(last_run: &BTreeSet<String>, notices: &[Notice]) -> Vec<String> {
    notices
        .iter()
        .filter(|n| !last_run.contains(&n.key))
        .map(|n| n.key.clone())
        .collect()
}

/// The keys the last run saved; `None` when they cannot be read, and then
/// nothing counts as news.
pub fn known_from_json(text: &str) -> Option<BTreeSet<String>> {
    let keys: Vec<String> = serde_json::from_str(text).ok()?;
    Some(
        keys.into_iter()
            .filter(|key| key.len() <= MAX_KNOWN_KEY)
            .take(MAX_KNOWN)
            .collect(),
    )
}

pub fn known_to_json(keys: &BTreeSet<String>) -> String {
    let keys: Vec<&String> = keys
        .iter()
        .filter(|key| key.len() <= MAX_KNOWN_KEY)
        .take(MAX_KNOWN)
        .collect();
    serde_json::to_string(&keys).unwrap_or_else(|_| "[]".into())
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
                " p{i}: resource(url: $u{i}) {{ ... on PullRequest {{ comments(last: {PLAN_COMMENTS}) {{ nodes {{ body }} }} }} }}"
            )
        })
        .collect();
    format!("query({}) {{{fields} }}", vars.join(", "))
}

/// What each pull request's latest Atlantis plan destroys, in the order asked
/// (see [`destroy_count`]); `None` for the whole answer when it is not a
/// GraphQL response, and for a pull request the account cannot read.
pub fn parse_plans(json: &str, count: usize) -> Option<Vec<Option<u32>>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let data = v.get("data")?;
    Some(
        (0..count)
            .map(|i| {
                let bodies: Vec<&str> = data[format!("p{i}")]["comments"]["nodes"]
                    .as_array()?
                    .iter()
                    .filter_map(|c| c["body"].as_str())
                    .collect();
                destroy_count(&bodies)
            })
            .collect(),
    )
}

/// Resources the latest Atlantis plan destroys, from a pull request's last
/// comments, oldest first: the "Plan: … N to destroy" lines of the last
/// comment that starts with "Ran Plan for" (a quote of one does not) and of
/// its "Continued…" parts. Atlantis repeats each project's summary line
/// outside its output block, so lines inside code blocks count first.
/// `None` when no plan is in sight: the caller keeps what it knew, since
/// later comments may have pushed the plan out of the window. Applies are
/// not read: a failed or partial apply must not hide what is left, and the
/// notice goes once a new plan destroys nothing or the pull request closes.
pub fn destroy_count(bodies: &[&str]) -> Option<u32> {
    let start = bodies
        .iter()
        .rposition(|b| b.trim_start().starts_with("Ran Plan for"))?;
    let parts = bodies[start..]
        .iter()
        .enumerate()
        .take_while(|(i, body)| *i == 0 || body.trim_start().starts_with("Continued"))
        .map(|(_, body)| body);
    let (mut inside, mut outside, mut seen_inside) = (0u32, 0u32, false);
    for body in parts {
        let mut fenced = false;
        for line in body.lines() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            let Some(destroy) = plan_destroys(line) else {
                continue;
            };
            if fenced {
                seen_inside = true;
                inside = inside.saturating_add(destroy);
            } else {
                outside = outside.saturating_add(destroy);
            }
        }
    }
    Some(if seen_inside { inside } else { outside })
}

/// "Plan: 1 to add, 0 to change, 2 to destroy." → 2, also behind a log
/// prefix ("16:47:01 STDOUT terraform: Plan: …", as Terragrunt prints it).
fn plan_destroys(line: &str) -> Option<u32> {
    let at = line.find("Plan: ")?;
    line[at + "Plan: ".len()..].split(',').find_map(|part| {
        part.trim()
            .trim_end_matches('.')
            .strip_suffix(" to destroy")?
            .trim()
            .parse()
            .ok()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanRead {
    /// The pull request's `updatedAt` when its plan was last read.
    updated_at: String,
    destroy: u32,
    tried_ms: u64,
}

/// What the latest plan of each pull request destroys, per account, read
/// again only once the pull request changed.
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

    /// Up to [`MAX_PLAN_FETCH`] pull requests (`url`, `updatedAt`) changed
    /// since their plan was read, the least recently tried first, so a few
    /// busy ones never keep the others waiting.
    pub fn due<'a>(
        &self,
        login: &str,
        pulls: impl IntoIterator<Item = &'a PullRequest>,
    ) -> Vec<(String, String)> {
        let mut due: Vec<(u64, &PullRequest)> = pulls
            .into_iter()
            .filter_map(|p| match self.reads.get(&Self::key(login, &p.url)) {
                Some(read) if read.updated_at == p.updated_at => None,
                read => Some((read.map_or(0, |r| r.tried_ms), p)),
            })
            .collect();
        due.sort_by_key(|(tried, _)| *tried);
        due.into_iter()
            .take(MAX_PLAN_FETCH)
            .map(|(_, p)| (p.url.clone(), p.updated_at.clone()))
            .collect()
    }

    /// The outcome of reading one pull request at `updated_at`: `None` when
    /// the request failed (read again later, after the others), `Some(None)`
    /// when no plan was in sight (what was known stands).
    pub fn record(
        &mut self,
        login: &str,
        url: &str,
        updated_at: &str,
        read: Option<Option<u32>>,
        now_ms: u64,
    ) {
        let entry = self
            .reads
            .entry(Self::key(login, url))
            .or_insert_with(|| PlanRead {
                updated_at: String::new(),
                destroy: 0,
                tried_ms: 0,
            });
        entry.tried_ms = now_ms;
        if let Some(destroy) = read {
            entry.updated_at = updated_at.into();
            if let Some(destroy) = destroy {
                entry.destroy = destroy;
            }
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

    pub fn destroy(&self, login: &str, url: &str) -> u32 {
        self.reads
            .get(&Self::key(login, url))
            .map_or(0, |read| read.destroy)
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
            destroy: 0,
            head: format!("feat/{number}"),
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
        destroys.destroy = 3;
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
                (NoticeKind::Critical, "infra #482 · 3 to destroy"),
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
            "critical:https://github.com/mt/infra/pull/482:3"
        );
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
    fn a_plan_destroys_what_its_latest_atlantis_comment_says() {
        let plan = |n: u32| {
            format!(
                "Ran Plan for dir: `vpn` workspace: `default`\n\n<details><summary>Show Output</summary>\n\n```diff\n- resource \"x\"\n\nPlan: 1 to add, 0 to change, {n} to destroy.\n```\n</details>\nPlan: 1 to add, 0 to change, {n} to destroy.\n"
            )
        };
        let (two, zero) = (plan(2), plan(0));
        assert_eq!(
            destroy_count(&[&two]),
            Some(2),
            "the line outside the block is the same plan"
        );
        assert_eq!(destroy_count(&[&two, "lgtm"]), Some(2));
        assert_eq!(
            destroy_count(&[&two, &zero]),
            Some(0),
            "the latest plan wins"
        );
        // Applies are not read: a failed or partial one must not hide anything.
        assert_eq!(
            destroy_count(&[&two, "Ran Apply for dir: `vpn`\n**Apply Error**"]),
            Some(2)
        );
        // A quote is not Atlantis.
        let quoted = format!("> {}", plan(5).replace('\n', "\n> "));
        assert_eq!(destroy_count(&[&two, &quoted]), Some(2));
        // No plan in sight: unknown, not zero.
        assert_eq!(
            destroy_count(&["lgtm", "Plan: 0 to add, 0 to change, 9 to destroy."]),
            None
        );
        let two_projects = "Ran Plan for 2 projects:\n```diff\nPlan: 0 to add, 0 to change, 1 to destroy.\n```\n```diff\nPlan: 0 to add, 0 to change, 1 to destroy.\n```\n";
        assert_eq!(destroy_count(&[two_projects]), Some(2));
        let split = [
            "Ran Plan for dir: `big`\n```diff\n- lots of output",
            "Continued plan output from previous comment.\n```diff\nPlan: 1 to import, 0 to add, 0 to change, 4 to destroy.\n```",
        ];
        assert_eq!(destroy_count(&split), Some(4));
        let terragrunt = "Ran Plan for dir: `live/db`\n```\n16:47:01.511 STDOUT terraform: Plan: 0 to add, 1 to change, 3 to destroy.\n```";
        assert_eq!(destroy_count(&[terragrunt]), Some(3));
    }

    #[test]
    fn plans_are_read_again_only_when_the_pull_request_changed_and_never_starve() {
        let mut cache = PlanCache::new();
        let mut pulls: Vec<PullRequest> = (1..=3).map(|n| pull("mt/infra", n)).collect();
        for p in &mut pulls {
            p.updated_at = "t1".into();
        }
        let due = cache.due("Me", &pulls);
        assert_eq!(due.len(), MAX_PLAN_FETCH);
        // #1 destroys 3; reading #2 failed.
        cache.record("me", &due[0].0, "t1", Some(Some(3)), 10);
        cache.record("me", &due[1].0, "t1", None, 10);
        assert_eq!(cache.destroy("ME", &pulls[0].url), 3);
        // #3 was never tried: it goes before #2, which failed.
        let due: Vec<_> = cache
            .due("me", &pulls)
            .into_iter()
            .map(|(u, _)| u)
            .collect();
        assert_eq!(due, [pulls[2].url.clone(), pulls[1].url.clone()]);
        // A change brings #1 back; a window without the plan keeps what was known.
        pulls[0].updated_at = "t2".into();
        assert!(cache
            .due("me", &pulls)
            .iter()
            .any(|(u, _)| *u == pulls[0].url));
        cache.record("me", &pulls[0].url, "t2", Some(None), 20);
        assert_eq!(cache.destroy("me", &pulls[0].url), 3);
        assert!(!cache
            .due("me", &pulls)
            .iter()
            .any(|(u, _)| *u == pulls[0].url));
        // Another account reads for itself.
        assert_eq!(cache.destroy("other", &pulls[0].url), 0);
        assert_eq!(cache.due("other", &pulls).len(), MAX_PLAN_FETCH);
    }

    #[test]
    fn the_plan_cache_forgets_the_least_recently_read_first() {
        let mut cache = PlanCache::new();
        for i in 0..=MAX_PLANS as u64 {
            cache.record("me", &format!("u{i}"), "t", Some(Some(1)), i + 1);
        }
        assert_eq!(cache.reads.len(), MAX_PLANS);
        assert_eq!(cache.destroy("me", "u0"), 0);
        assert_eq!(cache.destroy("me", &format!("u{MAX_PLANS}")), 1);
    }

    #[test]
    fn plan_comments_are_asked_through_variables_and_read_back_in_order() {
        assert_eq!(
            plan_query(2),
            "query($u0: URI!, $u1: URI!) { p0: resource(url: $u0) { ... on PullRequest { comments(last: 5) { nodes { body } } } } p1: resource(url: $u1) { ... on PullRequest { comments(last: 5) { nodes { body } } } } }"
        );
        let response = json!({"data": {
            "p0": {"comments": {"nodes": [{"body": "Ran Plan for dir: `a`\n```\nPlan: 0 to add, 0 to change, 2 to destroy.\n```"}]}},
            "p1": null
        }});
        assert_eq!(
            parse_plans(&response.to_string(), 2),
            Some(vec![Some(2), None])
        );
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

    #[test]
    fn what_was_not_there_at_the_last_run_is_news_and_the_keys_survive_a_round_trip() {
        let notice = |key: &str| Notice {
            key: key.into(),
            kind: NoticeKind::Review,
            detail: String::new(),
            url: String::new(),
            number: 0,
            title: String::new(),
        };
        let last: BTreeSet<String> = ["review:a".to_owned(), "action:1:t".to_owned()].into();
        let now = [notice("review:a"), notice("approved:b"), notice("failed:c")];
        assert_eq!(new_since(&last, &now), ["approved:b", "failed:c"]);
        assert_eq!(known_from_json(&known_to_json(&last)), Some(last));
    }

    #[test]
    fn saved_keys_are_capped_and_anything_else_reads_as_none() {
        let long = "x".repeat(MAX_KNOWN_KEY + 1);
        let many: Vec<String> = (0..MAX_KNOWN + 5).map(|i| format!("k{i}")).collect();
        let read = known_from_json(&serde_json::to_string(&many).unwrap()).unwrap();
        assert_eq!(read.len(), MAX_KNOWN);
        let read = known_from_json(&serde_json::to_string(&["ok", long.as_str()]).unwrap());
        assert_eq!(read, Some(["ok".to_owned()].into()));
        for bad in ["", "{}", "[1]", "null", "[\"a\""] {
            assert_eq!(known_from_json(bad), None, "{bad}");
        }
    }
}
