//! Bounded parsing of the fixed GitHub CLI response.
use serde::Serialize;
use serde_json::Value;

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
