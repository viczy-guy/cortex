//! What the island tells the user about GitHub (#131): a review requested, a
//! pull request ready or failing, a plan that destroys something, a failed
//! action. The shell fetches; everything here only reads what came back.

use crate::github::{Checks, GithubSummary, PullRequest, Review};
use serde::Serialize;
use serde_json::Value;

/// Pull requests whose comments are read per poll; the others wait for the next.
pub const MAX_PLAN_FETCH: usize = 3;
/// Comments read per pull request: an Atlantis plan and its continuations.
const PLAN_COMMENTS: usize = 5;
pub const MAX_RUNS: usize = 10;
/// Failed actions older than this are not news.
const RUNS_WINDOW_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_DETAIL_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    /// Someone asked for your review.
    Review,
    /// Your pull request is approved and its checks pass.
    Ready,
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
        });
    }
    for pull in &summary.review_requests {
        let who = pull.author.as_deref().unwrap_or("review");
        out.push(pr(NoticeKind::Review, "review", pull, who.into()));
    }
    for pull in &summary.mine {
        if !pull.draft
            && pull.review == Review::Approved
            && matches!(pull.checks, Checks::Passing | Checks::None)
        {
            out.push(pr(NoticeKind::Ready, "ready", pull, "approved".into()));
        }
    }
    out
}

fn short(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}

fn bounded(text: &str) -> String {
    let mut out: String = text.chars().take(MAX_DETAIL_CHARS).collect();
    if text.chars().count() > MAX_DETAIL_CHARS {
        out.push('…');
    }
    out
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
/// branch", which GitHub sends to whoever started the run.
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

/// What each pull request's latest Atlantis plan destroys, in the order asked;
/// `None` when the answer is not a GraphQL response.
pub fn parse_plans(json: &str, count: usize) -> Option<Vec<u32>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let data = v.get("data")?;
    Some(
        (0..count)
            .map(|i| {
                let bodies: Vec<&str> = data[format!("p{i}")]["comments"]["nodes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["body"].as_str())
                    .collect();
                destroy_count(&bodies)
            })
            .collect(),
    )
}

/// Resources the latest Atlantis plan destroys, from a pull request's last
/// comments, oldest first: the "Plan: … N to destroy" lines of the last "Ran
/// Plan for" comment and its "Continued…" parts. Zero when nothing was
/// planned, or once a later "Ran Apply for" applied it. Atlantis repeats each
/// project's summary line outside its output block, so lines inside code
/// blocks count first.
pub fn destroy_count(bodies: &[&str]) -> u32 {
    let Some(start) = bodies.iter().rposition(|b| b.contains("Ran Plan for")) else {
        return 0;
    };
    if bodies[start + 1..]
        .iter()
        .any(|b| b.contains("Ran Apply for"))
    {
        return 0;
    }
    let parts = bodies[start..]
        .iter()
        .enumerate()
        .take_while(|(i, body)| *i == 0 || body.trim_start().starts_with("Continued"))
        .map(|(_, body)| body);
    let (mut inside, mut outside, mut seen_inside) = (0u32, 0u32, false);
    for body in parts {
        let mut fenced = false;
        for line in body.lines() {
            let line = line.trim_start_matches(|c: char| c.is_whitespace() || c == '+' || c == '-');
            if line.starts_with("```") {
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
    if seen_inside {
        inside
    } else {
        outside
    }
}

/// "Plan: 1 to add, 0 to change, 2 to destroy." → 2.
fn plan_destroys(line: &str) -> Option<u32> {
    line.strip_prefix("Plan:")?.split(',').find_map(|part| {
        part.trim()
            .trim_end_matches('.')
            .strip_suffix(" to destroy")?
            .trim()
            .parse()
            .ok()
    })
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
        let mut ready = pull("viczy-guy/cortex", 130);
        ready.review = Review::Approved;
        ready.checks = Checks::Passing;
        let mut draft_ready = ready.clone();
        draft_ready.number = 131;
        draft_ready.url.push('1');
        draft_ready.draft = true;
        let waiting = pull("viczy-guy/cortex", 132);
        let mut destroys = pull("mt/infra", 482);
        destroys.destroy = 3;
        s.mine = vec![failing, ready, draft_ready, waiting];
        s.review_requests = vec![destroys];
        s.failed_runs = vec![FailedRun {
            id: "27485366758".into(),
            repo: "viczy-guy/cortex".into(),
            workflow: "CI".into(),
            reference: "main".into(),
            updated_at: "2026-11-23T16:38:01Z".into(),
        }];
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
                (NoticeKind::Ready, "cortex #130 · approved"),
            ]
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
    fn a_plan_destroys_what_its_latest_comment_says_until_it_is_applied() {
        let plan = |n: u32| {
            format!(
                "Ran Plan for dir: `vpn` workspace: `default`\n\n<details><summary>Show Output</summary>\n\n```diff\n- resource \"x\"\n\nPlan: 1 to add, 0 to change, {n} to destroy.\n```\n</details>\nPlan: 1 to add, 0 to change, {n} to destroy.\n"
            )
        };
        let two = plan(2);
        let zero = plan(0);
        assert_eq!(
            destroy_count(&[&two]),
            2,
            "the line outside the block is the same plan"
        );
        assert_eq!(destroy_count(&[&two, "lgtm"]), 2);
        assert_eq!(destroy_count(&[&two, &zero]), 0, "the latest plan wins");
        assert_eq!(destroy_count(&[&two, "Ran Apply for dir: `vpn`"]), 0);
        assert_eq!(
            destroy_count(&["no plan here", "Plan: 0 to add, 0 to change, 9 to destroy."]),
            0
        );
        let two_projects = "Ran Plan for 2 projects:\n```diff\nPlan: 0 to add, 0 to change, 1 to destroy.\n```\n```diff\nPlan: 0 to add, 0 to change, 1 to destroy.\n```\n";
        assert_eq!(destroy_count(&[two_projects]), 2);
        let split = [
            "Ran Plan for dir: `big`\n```diff\n- lots of output",
            "Continued plan output from previous comment.\n```diff\nPlan: 1 to import, 0 to add, 0 to change, 4 to destroy.\n```",
        ];
        assert_eq!(destroy_count(&split), 4);
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
        assert_eq!(parse_plans(&response.to_string(), 2), Some(vec![2, 0]));
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
    fn repository_names_are_owner_and_name_only() {
        assert!(is_repo_name("viczy-guy/cortex"));
        assert!(is_repo_name("mt/infra.tf_v2"));
        for bad in [
            "cortex", "a/b/c", "../etc", "a/..", "a/", "/b", "a b/c", "a/b?x=1",
        ] {
            assert!(!is_repo_name(bad), "{bad}");
        }
    }
}
