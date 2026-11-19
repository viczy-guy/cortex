//! Bounded parsing of the fixed GitHub CLI response.
use serde::Serialize;
use serde_json::Value;

pub const MAX_ACCOUNTS: usize = 10;
pub const DEVICE_URL: &str = "https://github.com/login/device";
pub const CLI_URL: &str = "https://cli.github.com/";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GhAccount {
    pub login: String,
    pub active: bool,
    pub valid: bool,
}

/// Enterprise Managed Users add `_` and their enterprise's short code
/// (`mona-cat_octo`). A login must never start like a command-line flag.
pub fn is_valid_login(login: &str) -> bool {
    login.len() <= 39
        && login
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn is_valid_token(token: &str) -> bool {
    (1..=255).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn github_hosts(value: &Value) -> Option<&[Value]> {
    match value.get("hosts")?.as_object()?.get("github.com") {
        Some(host) => Some(host.as_array()?.as_slice()),
        None => Some(&[]),
    }
}

fn account(entry: &Value) -> Option<GhAccount> {
    let login = entry["login"].as_str().filter(|s| is_valid_login(s))?;
    Some(GhAccount {
        login: login.into(),
        active: entry["active"].as_bool()?,
        valid: entry["state"].as_str()? == "success",
    })
}

pub fn parse_accounts(json: &str) -> Option<Vec<GhAccount>> {
    let value = serde_json::from_str(json).ok()?;
    let mut accounts: Vec<GhAccount> = Vec::new();
    for entry in github_hosts(&value)?.iter().filter_map(account) {
        if !accounts
            .iter()
            .any(|a| a.login.eq_ignore_ascii_case(&entry.login))
        {
            accounts.push(entry);
            if accounts.len() == MAX_ACCOUNTS {
                break;
            }
        }
    }
    Some(accounts)
}

/// Account restoration must also work when a newly added account displaces
/// the previous one from the capped display list.
pub fn has_account(json: &str, login: &str) -> Option<bool> {
    let value = serde_json::from_str(json).ok()?;
    Some(
        github_hosts(&value)?
            .iter()
            .filter_map(account)
            .any(|a| a.login.eq_ignore_ascii_case(login)),
    )
}

pub fn active_account(json: &str) -> Option<Option<String>> {
    let value = serde_json::from_str(json).ok()?;
    match github_hosts(&value)?
        .iter()
        .find(|entry| entry["active"].as_bool() == Some(true))
    {
        Some(entry) => Some(Some(account(entry)?.login)),
        None => Some(None),
    }
}

pub fn device_code(line: &str) -> Option<String> {
    line.as_bytes().windows(9).find_map(|candidate| {
        (candidate[4] == b'-'
            && candidate[..4]
                .iter()
                .chain(&candidate[5..])
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
        .then(|| String::from_utf8_lossy(candidate).into_owned())
    })
}

pub fn account_is_shown(hidden: &[String], login: &str) -> bool {
    !hidden.iter().any(|name| name.eq_ignore_ascii_case(login))
}

/// "ok" when a shown account fetched or every account is hidden; otherwise
/// "error" when a shown account failed, else "signed_out" (sign-in fixes it).
pub fn overall_state(accounts: impl Iterator<Item = &'static str>) -> &'static str {
    let (mut any, mut shown, mut failed) = (false, false, false);
    for account in accounts {
        any = true;
        match account {
            "ok" => return "ok",
            "hidden" => {}
            "error" => (shown, failed) = (true, true),
            _ => shown = true,
        }
    }
    if failed {
        "error"
    } else if any && !shown {
        "ok"
    } else {
        "signed_out"
    }
}

pub const MAX_PULLS: usize = 20;
const MAX_TITLE_CHARS: usize = 200;
const MAX_REPO_CHARS: usize = 140;
const MAX_LOGIN_CHARS: usize = 64;
/// The one query Cortex sends through `gh api graphql`.
pub const QUERY: &str = r#"query {
  viewer { login }
  mine: search(query: "is:open is:pr author:@me archived:false sort:updated-desc", type: ISSUE, first: 20) {
    issueCount
    nodes { ... on PullRequest { number title url isDraft reviewDecision repository { nameWithOwner } author { login }
      commits(last: 1) { nodes { commit { statusCheckRollup { state } } } } } }
  }
  reviews: search(query: "is:open is:pr review-requested:@me archived:false sort:updated-desc", type: ISSUE, first: 20) {
    issueCount
    nodes { ... on PullRequest { number title url isDraft reviewDecision repository { nameWithOwner } author { login }
      commits(last: 1) { nodes { commit { statusCheckRollup { state } } } } } }
  }
}"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Checks {
    Passing,
    Failing,
    Pending,
    None,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    Approved,
    ChangesRequested,
    Required,
    None,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PullRequest {
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub draft: bool,
    pub author: Option<String>,
    pub checks: Checks,
    pub review: Review,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GithubSummary {
    pub login: String,
    pub mine: Vec<PullRequest>,
    pub mine_total: u32,
    pub review_requests: Vec<PullRequest>,
    pub review_requests_total: u32,
    pub fetched_at_ms: u64,
}
impl GithubSummary {
    /// Own pull requests whose checks are failing.
    pub fn failing(&self) -> usize {
        self.mine
            .iter()
            .filter(|p| p.checks == Checks::Failing)
            .count()
    }
    /// Whether the URL is one of the links in this summary.
    pub fn has_url(&self, url: &str) -> bool {
        self.mine
            .iter()
            .chain(&self.review_requests)
            .any(|p| p.url == url)
    }
}
fn count(v: &Value) -> u32 {
    v["issueCount"].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32
}
fn pulls(v: &Value) -> Vec<PullRequest> {
    v["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| {
            let number = n["number"].as_u64().filter(|n| *n > 0)?;
            let title = n["title"].as_str()?;
            let repo = n["repository"]["nameWithOwner"].as_str()?;
            if repo.is_empty() || repo.chars().count() > MAX_REPO_CHARS {
                return None;
            }
            let url = n["url"].as_str()?;
            if !url.starts_with("https://github.com/")
                || url.chars().count() > 512
                || url.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return None;
            }
            let clean: String = title
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            let clean = clean.trim();
            let mut title: String = clean.chars().take(MAX_TITLE_CHARS).collect();
            if clean.chars().count() > MAX_TITLE_CHARS {
                title.push('…');
            }
            let checks = match n
                .pointer("/commits/nodes/0/commit/statusCheckRollup/state")
                .and_then(Value::as_str)
            {
                Some("SUCCESS") => Checks::Passing,
                Some("FAILURE" | "ERROR") => Checks::Failing,
                Some("PENDING" | "EXPECTED") => Checks::Pending,
                _ => Checks::None,
            };
            let review = match n["reviewDecision"].as_str() {
                Some("APPROVED") => Review::Approved,
                Some("CHANGES_REQUESTED") => Review::ChangesRequested,
                Some("REVIEW_REQUIRED") => Review::Required,
                _ => Review::None,
            };
            Some(PullRequest {
                repo: repo.into(),
                number,
                title,
                url: url.into(),
                draft: n["isDraft"].as_bool().unwrap_or(false),
                author: n["author"]["login"]
                    .as_str()
                    .filter(|s| s.chars().count() <= MAX_LOGIN_CHARS)
                    .map(str::to_owned),
                checks,
                review,
            })
        })
        .take(MAX_PULLS)
        .collect()
}
/// Parse the stdout of the fixed `gh api graphql` query.
/// Parse stdout from the fixed `gh api graphql` query.
pub fn parse_summary(json: &str, now_ms: u64) -> Option<GithubSummary> {
    let v: Value = serde_json::from_str(json).ok()?;
    let d = v.get("data")?;
    let login = d["viewer"]["login"].as_str()?;
    if login.is_empty() || login.chars().count() > MAX_LOGIN_CHARS {
        return None;
    }
    let mine = d.get("mine").filter(|v| v.is_object())?;
    let reviews = d.get("reviews").filter(|v| v.is_object())?;
    Some(GithubSummary {
        login: login.into(),
        mine: pulls(mine),
        mine_total: count(mine),
        review_requests: pulls(reviews),
        review_requests_total: count(reviews),
        fetched_at_ms: now_ms,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn github_accounts_include_broken_accounts_and_skip_invalid_entries() {
        let input = json!({"hosts":{"github.com":[
            {"state":"success","active":true,"host":"github.com","login":"alice","tokenSource":"keyring","scopes":"repo","gitProtocol":"https"},
            {"state":"error","active":false,"host":"github.com","login":"bob"},
            {"state":"success","active":false,"login":"Alice"},
            {"state":"success","active":false,"login":"-x"},
            {"state":"success","active":"yes","login":"carol"}
        ],"other.example":[{"state":"success","active":true,"login":"other"}]}});
        assert_eq!(
            parse_accounts(&input.to_string()).unwrap(),
            [
                GhAccount {
                    login: "alice".into(),
                    active: true,
                    valid: true
                },
                GhAccount {
                    login: "bob".into(),
                    active: false,
                    valid: false
                },
            ]
        );
        assert_eq!(parse_accounts(r#"{"hosts":{}}"#), Some(vec![]));
        assert_eq!(parse_accounts("{}"), None);
        assert_eq!(parse_accounts("invalid"), None);
    }

    #[test]
    fn account_restoration_can_find_an_account_outside_the_display_limit() {
        let accounts: Vec<_> = (0..=MAX_ACCOUNTS).map(|i| {
            json!({"login":format!("user{i}"),"active":i == MAX_ACCOUNTS,"state":"success"})
        }).collect();
        let input = json!({"hosts":{"github.com":accounts}}).to_string();
        let active = format!("user{MAX_ACCOUNTS}");
        let displayed = parse_accounts(&input).unwrap();
        assert_eq!(displayed.len(), MAX_ACCOUNTS);
        assert!(displayed.iter().all(|account| !account.active));
        assert_eq!(has_account(&input, &active), Some(true));
        assert_eq!(active_account(&input), Some(Some(active)));
        assert_eq!(
            active_account(
                r#"{"hosts":{"github.com":[{"login":"-x","active":true,"state":"success"}]}}"#
            ),
            None
        );
    }

    #[test]
    fn credentials_reject_invalid_characters_and_lengths() {
        for login in ["-x", "_x", "a b", "a/b", "", &"a".repeat(40)] {
            assert!(!is_valid_login(login));
        }
        for token in ["a-b", "a b", "", &"a".repeat(256)] {
            assert!(!is_valid_token(token));
        }
        assert!(is_valid_login("a-123"));
        assert!(is_valid_token("github_pat_123"));
    }

    #[test]
    fn an_enterprise_managed_user_is_listed_and_can_be_the_active_account() {
        let input = json!({"hosts":{"github.com":[
            {"state":"success","active":true,"login":"mona-cat_octo"},
            {"state":"success","active":false,"login":"alice"}
        ]}})
        .to_string();
        let logins: Vec<_> = parse_accounts(&input)
            .unwrap()
            .into_iter()
            .map(|a| a.login)
            .collect();
        assert_eq!(logins, ["mona-cat_octo", "alice"]);
        assert_eq!(active_account(&input), Some(Some("mona-cat_octo".into())));
        assert_eq!(has_account(&input, "mona-cat_octo"), Some(true));
    }

    #[test]
    fn both_sign_in_formats_yield_a_device_code() {
        for line in [
            "! One-time code (ABCD-1234) copied to clipboard",
            "! First copy your one-time code: ABCD-1234",
        ] {
            assert_eq!(device_code(line).as_deref(), Some("ABCD-1234"));
        }
        assert_eq!(device_code("Open this URL to continue"), None);
    }

    #[test]
    fn hidden_accounts_do_not_count_as_failed_fetches() {
        assert_eq!(overall_state([].into_iter()), "signed_out");
        assert_eq!(overall_state(["hidden"].into_iter()), "ok");
        assert_eq!(overall_state(["error", "hidden"].into_iter()), "error");
        assert_eq!(overall_state(["error", "ok"].into_iter()), "ok");
    }

    #[test]
    fn an_account_that_needs_sign_in_is_not_a_failed_request() {
        assert_eq!(overall_state(["signed_out"].into_iter()), "signed_out");
        assert_eq!(
            overall_state(["hidden", "signed_out"].into_iter()),
            "signed_out"
        );
        assert_eq!(overall_state(["signed_out", "error"].into_iter()), "error");
        assert_eq!(overall_state(["signed_out", "ok"].into_iter()), "ok");
        assert!(!account_is_shown(&["Alice".into()], "alice"));
        assert!(account_is_shown(&["Alice".into()], "bob"));
    }

    fn node() -> Value {
        json!({"number":42,"title":"Fix it","url":"https://github.com/o/r/pull/42","isDraft":true,"repository":{"nameWithOwner":"o/r"},"author":{"login":"alice"},"reviewDecision":"APPROVED","commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"FAILURE"}}}]}})
    }
    fn response(nodes: Vec<Value>) -> Value {
        json!({"data":{"viewer":{"login":"alice"},"mine":{"issueCount":25,"nodes":nodes},"reviews":{"issueCount":0,"nodes":[]}}})
    }
    #[test]
    fn a_realistic_response_preserves_fields_and_links() {
        let s = parse_summary(&response(vec![node()]).to_string(), 12).unwrap();
        assert_eq!(s.login, "alice");
        assert_eq!(s.mine_total, 25);
        assert_eq!(s.fetched_at_ms, 12);
        let p = &s.mine[0];
        assert_eq!(p.repo, "o/r");
        assert_eq!(p.number, 42);
        assert_eq!(p.title, "Fix it");
        assert!(p.draft);
        assert_eq!(p.author.as_deref(), Some("alice"));
        assert_eq!(p.review, Review::Approved);
        assert_eq!(s.failing(), 1);
        assert!(s.has_url(&p.url));
        assert!(!s.has_url("https://github.com/other"));
    }
    #[test]
    fn an_empty_response_has_no_pulls() {
        let mut v = response(vec![]);
        v["data"]["mine"]["issueCount"] = json!(0);
        let s = parse_summary(&v.to_string(), 0).unwrap();
        assert_eq!(s.mine_total, 0);
        assert_eq!(s.review_requests_total, 0);
        assert!(s.review_requests.is_empty());
        assert!(s.mine.is_empty());
        assert_eq!(s.failing(), 0);
    }
    #[test]
    fn malformed_nodes_and_foreign_urls_are_skipped() {
        let mut nodes = vec![json!({}), json!(null)];
        for url in [
            "http://github.com/o/r",
            "https://github.com.evil.com/o/r",
            "https://github.com/o/ r",
        ] {
            let mut n = node();
            n["url"] = json!(url);
            nodes.push(n);
        }
        nodes.push(node());
        assert_eq!(
            parse_summary(&response(nodes).to_string(), 0)
                .unwrap()
                .mine
                .len(),
            1
        );
    }
    #[test]
    fn titles_are_cleaned_and_cut_on_character_boundaries() {
        let mut n = node();
        n["title"] = json!(format!("\n{}\t", "é".repeat(201)));
        let s = parse_summary(&response(vec![n]).to_string(), 0).unwrap();
        assert_eq!(s.mine[0].title, format!("{}…", "é".repeat(200)));
    }
    #[test]
    fn a_missing_viewer_is_rejected() {
        assert!(parse_summary("{}", 0).is_none());
        let mut v = response(vec![]);
        v["data"]["viewer"] = json!(null);
        assert!(parse_summary(&v.to_string(), 0).is_none());
    }
    #[test]
    fn the_query_and_parser_share_the_list_limit() {
        assert_eq!(MAX_PULLS, 20);
        assert_eq!(QUERY.matches("first: 20").count(), 2);
        assert_eq!(
            parse_summary(&response(vec![node(); 21]).to_string(), 0)
                .unwrap()
                .mine
                .len(),
            20
        );
    }
    #[test]
    fn counts_saturate_and_partial_graphql_errors_preserve_complete_data() {
        let mut v = response(vec![]);
        v["errors"] = json!([{"message":"partial"}]);
        v["data"]["mine"]["issueCount"] = json!(u64::MAX);
        v["data"]["reviews"] = json!({"nodes":[node()]});
        let s = parse_summary(&v.to_string(), 0).unwrap();
        assert_eq!(s.mine_total, u32::MAX);
        assert_eq!(s.review_requests_total, 0);
        assert_eq!(s.review_requests.len(), 1);
        assert!(s.has_url(&s.review_requests[0].url));
        for count in [json!(-1), json!(1.5), json!("1"), json!(null)] {
            v["data"]["mine"]["issueCount"] = count;
            assert_eq!(parse_summary(&v.to_string(), 0).unwrap().mine_total, 0);
        }
    }
    #[test]
    fn required_shapes_and_field_limits_are_enforced() {
        assert!(parse_summary("not json", 0).is_none());
        for field in ["mine", "reviews"] {
            for bad in [json!(null), json!([]), json!("wrong")] {
                let mut v = response(vec![]);
                v["data"][field] = bad;
                assert!(parse_summary(&v.to_string(), 0).is_none());
            }
        }
        for login in ["".to_owned(), "x".repeat(65)] {
            let mut v = response(vec![]);
            v["data"]["viewer"]["login"] = json!(login);
            assert!(parse_summary(&v.to_string(), 0).is_none());
        }
        for (pointer, value) in [
            ("/number", json!(0)),
            ("/number", json!(-1)),
            ("/number", json!(1.5)),
            ("/title", json!(false)),
            ("/repository/nameWithOwner", json!("")),
            ("/repository/nameWithOwner", json!("é".repeat(141))),
            (
                "/url",
                json!(format!("https://github.com/{}", "a".repeat(500))),
            ),
        ] {
            let mut n = node();
            *n.pointer_mut(pointer).unwrap() = value;
            assert!(parse_summary(&response(vec![n]).to_string(), 0)
                .unwrap()
                .mine
                .is_empty());
        }
        let mut n = node();
        n["author"]["login"] = json!("é".repeat(65));
        n["title"] = json!("  a\tb\nc  ");
        let s = parse_summary(&response(vec![n]).to_string(), 0).unwrap();
        assert_eq!(s.mine[0].author, None);
        assert_eq!(s.mine[0].title, "a b c");
    }
    #[test]
    fn checks_and_review_decisions_map_all_known_states() {
        for (raw, expected) in [
            ("SUCCESS", Checks::Passing),
            ("FAILURE", Checks::Failing),
            ("ERROR", Checks::Failing),
            ("PENDING", Checks::Pending),
            ("EXPECTED", Checks::Pending),
            ("UNKNOWN", Checks::None),
        ] {
            let mut n = node();
            *n.pointer_mut("/commits/nodes/0/commit/statusCheckRollup/state")
                .unwrap() = json!(raw);
            assert_eq!(
                parse_summary(&response(vec![n]).to_string(), 0)
                    .unwrap()
                    .mine[0]
                    .checks,
                expected
            );
        }
        for (raw, expected) in [
            ("APPROVED", Review::Approved),
            ("CHANGES_REQUESTED", Review::ChangesRequested),
            ("REVIEW_REQUIRED", Review::Required),
            ("UNKNOWN", Review::None),
        ] {
            let mut n = node();
            n["reviewDecision"] = json!(raw);
            n["commits"] = json!(null);
            n["author"] = json!(null);
            let s = parse_summary(&response(vec![n]).to_string(), 0).unwrap();
            assert_eq!(s.mine[0].review, expected);
            assert_eq!(s.mine[0].checks, Checks::None);
            assert_eq!(s.mine[0].author, None);
        }
    }
}
