//! Atlantis on pull requests (#161): what the latest plan adds, changes,
//! replaces and destroys. The commit statuses tell where the run stands and
//! the counts, at every check; the plan comment tells the detail, read once
//! the pull request changes. Everything here only reads what came back.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Folders whose name makes an environment production, as the maintainer
/// set them (24/11/2026, "for now").
const PRODUCTION: [&str; 5] = ["production", "prod", "prd", "infraestructure", "network"];
/// Destroying or replacing one of these loses data or keys.
const DATA: [&str; 21] = [
    "aws_backup_vault",
    "aws_cognito_user_pool",
    "aws_db_instance",
    "aws_docdb_cluster",
    "aws_dynamodb_table",
    "aws_ebs_volume",
    "aws_ecr_repository",
    "aws_efs_file_system",
    "aws_elasticache_cluster",
    "aws_elasticache_replication_group",
    "aws_elasticsearch_domain",
    "aws_kinesis_stream",
    "aws_kms_key",
    "aws_msk_cluster",
    "aws_neptune_cluster",
    "aws_opensearch_domain",
    "aws_rds_cluster",
    "aws_redshift_cluster",
    "aws_route53_zone",
    "aws_s3_bucket",
    "aws_secretsmanager_secret",
];
const MAX_NAME_CHARS: usize = 300;
const MAX_ERROR_CHARS: usize = 200;
/// Groups kept per project, and in a whole plan; the rest is counted only.
const MAX_GROUPS: usize = 60;

/// Where a pull request's latest Atlantis run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Planning,
    Planned,
    PlanFailed,
    /// Every failed project waits for another pull request's lock.
    Locked,
    Applying,
    Applied,
    ApplyFailed,
}

/// What a plan does to one resource, the most dangerous first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Destroy,
    Replace,
    Change,
    Add,
}

/// Changes of one kind, in one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    /// The project, when the plan has several with changes.
    pub project: Option<String>,
    pub op: Op,
    /// The resource type: "aws_instance".
    pub kind: String,
    pub count: u32,
    /// The first resource's address, shown when it is alone.
    pub address: String,
}

/// A pull request's latest Atlantis run, as the island shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub stage: Stage,
    pub add: u32,
    pub change: u32,
    pub replace: u32,
    pub destroy: u32,
    /// Projects planned so far (every one, once planned), and those of them
    /// without changes.
    pub projects: u32,
    pub unchanged: u32,
    /// The environment shown: production first, else the first one found.
    pub env: Option<String>,
    pub production: bool,
    /// Resource types whose data a destroy or a replace loses.
    pub data: Vec<String>,
    /// Objects changed outside Terraform: the apply undoes it.
    pub drift: bool,
    pub groups: Vec<Group>,
    /// The counts come from the statuses: the list shows part of the plan.
    pub partial: bool,
    pub error: Option<String>,
    pub locked_by: Option<u64>,
    /// The plan's comment on GitHub.
    pub comment_url: Option<String>,
    /// When this run's stage began, as GitHub writes it: a new run, a new notice.
    #[serde(skip)]
    pub at: String,
    /// When its plan ran: the comments read are only ever shown with it.
    #[serde(skip)]
    pub planned_at: String,
}

/// The run as the statuses of the pull request's last commit tell it;
/// `None` without Atlantis.
pub fn from_statuses(contexts: &Value) -> Option<Plan> {
    let mut plan_run: Option<(&str, &str)> = None;
    let mut apply_run: Option<(&str, &str)> = None;
    let (mut add, mut change, mut destroy) = (0u32, 0u32, 0u32);
    let (mut projects, mut unchanged, mut failed) = (0u32, 0u32, 0u32);
    let mut envs = Vec::new();
    for c in contexts.as_array().into_iter().flatten() {
        let (Some(name), Some(state)) = (c["context"].as_str(), c["state"].as_str()) else {
            continue;
        };
        let at = c["createdAt"]
            .as_str()
            .filter(|t| t.len() <= 25)
            .unwrap_or("");
        match name {
            "atlantis/plan" => plan_run = Some((state, at)),
            "atlantis/apply" => apply_run = Some((state, at)),
            _ => {
                let Some(project) = name.strip_prefix("atlantis/plan: ") else {
                    continue;
                };
                if project.chars().count() > MAX_NAME_CHARS {
                    continue;
                }
                if let Some(env) = environment(project) {
                    envs.push(env.to_owned());
                }
                match state {
                    "SUCCESS" => {
                        projects += 1;
                        let description = c["description"].as_str().unwrap_or("");
                        match plan_counts(description) {
                            Some((a, ch, d)) if a + ch + d > 0 => {
                                add = add.saturating_add(a);
                                change = change.saturating_add(ch);
                                destroy = destroy.saturating_add(d);
                            }
                            _ => unchanged += 1,
                        }
                    }
                    "FAILURE" | "ERROR" => {
                        projects += 1;
                        failed += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    let (plan_state, plan_at) = plan_run?;
    let (stage, at) = match apply_run {
        // An apply after the latest plan; a plan run again later wins.
        Some((state, at)) if at >= plan_at => (
            match state {
                "PENDING" | "EXPECTED" => Stage::Applying,
                "SUCCESS" => Stage::Applied,
                _ => Stage::ApplyFailed,
            },
            at,
        ),
        _ => (
            match plan_state {
                "PENDING" | "EXPECTED" => Stage::Planning,
                "SUCCESS" if failed == 0 => Stage::Planned,
                "SUCCESS" => Stage::PlanFailed,
                _ => Stage::PlanFailed,
            },
            plan_at,
        ),
    };
    let production = envs.iter().any(|e| is_production(e));
    let env = envs
        .iter()
        .find(|e| is_production(e))
        .or(envs.first())
        .cloned();
    Some(Plan {
        stage,
        add,
        change,
        replace: 0,
        destroy,
        projects,
        unchanged,
        env,
        production,
        data: Vec::new(),
        drift: false,
        groups: Vec::new(),
        partial: add + change + destroy > 0,
        error: None,
        locked_by: None,
        comment_url: None,
        at: at.into(),
        planned_at: plan_at.into(),
    })
}

/// "Plan: 1 to import, 2 to add, 0 to change, 1 to destroy." (also behind a
/// log prefix) → (add, change, destroy); imports change nothing.
fn plan_counts(text: &str) -> Option<(u32, u32, u32)> {
    let at = text.find("Plan: ")?;
    let (mut add, mut change, mut destroy) = (0, 0, 0);
    for part in text[at + "Plan: ".len()..].split(',') {
        let part = part.trim().trim_end_matches('.');
        let (n, what) = part.split_once(' ')?;
        let n: u32 = n.parse().ok()?;
        match what {
            "to add" => add = n,
            "to change" => change = n,
            "to destroy" => destroy = n,
            "to import" | "to forget" => {}
            _ => return None,
        }
    }
    Some((add, change, destroy))
}

/// The environment of a project, by its folder or its name (Atlantis joins
/// the folders of a generated name with `_`): the folder right before the
/// region (`…/production/sa-east-1/…`). Without a region, a production
/// folder anywhere. A root folder or a sub-folder that happens to share a
/// production name (`infraestructure/…`, `…/networking/…`) is no environment.
pub fn environment(path: &str) -> Option<&str> {
    let parts: Vec<&str> = if path.contains('/') {
        path.split('/').collect()
    } else {
        path.split('_').collect()
    };
    if let Some(i) = parts.iter().position(|p| is_region(p)) {
        return parts[..i].last().copied().filter(|p| !p.is_empty());
    }
    parts.into_iter().find(|p| is_production(p))
}

pub fn is_production(env: &str) -> bool {
    PRODUCTION.iter().any(|p| p.eq_ignore_ascii_case(env))
}

/// "sa-east-1", "us-gov-west-1": letters, then words, then a number.
fn is_region(part: &str) -> bool {
    let words: Vec<&str> = part.split('-').collect();
    words.len() >= 3
        && words[0].len() == 2
        && words[..words.len() - 1]
            .iter()
            .all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase()))
        && words[words.len() - 1].bytes().all(|b| b.is_ascii_digit())
        && !words[words.len() - 1].is_empty()
}

/// One project of a plan or an apply comment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub dir: String,
    /// (op, resource type) → (count, first address).
    pub changes: BTreeMap<(Op, String), (u32, String)>,
    pub no_changes: bool,
    pub drift: bool,
    pub error: Option<String>,
    pub locked_by: Option<u64>,
}

/// The latest plan, or apply, out of a pull request's last comments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    pub url: String,
    pub projects: Vec<Project>,
    /// Its beginning was out of the comments read, or it had too much.
    pub cut: bool,
}

/// The pull request's latest plan and apply comments, as far as they were read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    pub plan: Option<Section>,
    pub apply: Option<Section>,
}

/// Read a pull request's last comments (`body`, `url`), oldest first.
pub fn read_comments(comments: &[(&str, &str)]) -> Detail {
    Detail {
        plan: section(comments, "Ran Plan for", "Continued plan output"),
        apply: section(comments, "Ran Apply for", "Continued apply output"),
    }
}

fn section(comments: &[(&str, &str)], start: &str, continued: &str) -> Option<Section> {
    let begins = |body: &str| body.trim_start().starts_with(start);
    let continues = |body: &str| {
        let body = body.trim_start();
        body.starts_with(continued) || body.starts_with("Continued from previous comment")
    };
    let (from, cut) = match comments.iter().rposition(|(body, _)| begins(body)) {
        Some(i) => (i, false),
        // The beginning went out of reach: what is left still counts.
        None => (comments.iter().position(|(body, _)| continues(body))?, true),
    };
    let parts: Vec<&(&str, &str)> = comments[from..]
        .iter()
        .enumerate()
        .take_while(|(i, (body, _))| *i == 0 || continues(body))
        .map(|(_, c)| c)
        .collect();
    let url = parts
        .first()
        .map(|(_, url)| *url)
        .filter(|u| is_github_url(u))?;
    let mut read = Section {
        url: url.into(),
        projects: Vec::new(),
        cut,
    };
    'parts: for (body, _) in parts {
        // Each comment is a document of its own: its fences open and close in it.
        let mut fenced = false;
        // After "**Plan Error**": its block's first "Error: …" line, else its first line.
        let mut error_block = false;
        let mut plain_error = false;
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                fenced = !fenced;
                if !fenced {
                    error_block = false;
                }
                continue;
            }
            if !fenced {
                if let Some(project) = header(trimmed) {
                    read.projects.push(project);
                    continue;
                }
                if trimmed.starts_with("### Plan Summary")
                    || trimmed.starts_with("### Apply Summary")
                {
                    break 'parts;
                }
            }
            let words = without_log_prefix(line).trim();
            if words.is_empty() {
                continue;
            }
            if let Some(n) = locked_by(words) {
                current(&mut read.projects).locked_by.get_or_insert(n);
            }
            if words.contains("Objects have changed outside of Terraform") {
                current(&mut read.projects).drift = true;
            }
            if words.starts_with("No changes.") {
                current(&mut read.projects).no_changes = true;
            }
            if let Some(reason) = words
                .strip_prefix("**Plan Failed**:")
                .or_else(|| words.strip_prefix("**Apply Failed**:"))
            {
                current(&mut read.projects)
                    .error
                    .get_or_insert_with(|| clip(reason));
                continue;
            }
            if words.starts_with("**Plan Error**") || words.starts_with("**Apply Error**") {
                error_block = true;
                continue;
            }
            if !fenced {
                continue;
            }
            if error_block {
                let project = current(&mut read.projects);
                let text = words.trim_start_matches(['│', '╷', '╵', ' ']);
                if let Some(at) = text.find("Error: ") {
                    if project.error.is_none() || plain_error {
                        project.error = Some(clip(&text[at + "Error: ".len()..]));
                        plain_error = false;
                    }
                } else if project.error.is_none() && !text.is_empty() {
                    project.error = Some(clip(text));
                    plain_error = true;
                }
                continue;
            }
            if let Some((op, address)) = change_line(words) {
                let project = current(&mut read.projects);
                let key = (op, resource_type(address).to_owned());
                if !project.changes.contains_key(&key) && project.changes.len() >= MAX_GROUPS {
                    read.cut = true;
                    continue;
                }
                let (count, _) = project
                    .changes
                    .entry(key)
                    .or_insert_with(|| (0, address.chars().take(MAX_NAME_CHARS).collect()));
                *count = count.saturating_add(1);
            }
        }
    }
    Some(read)
}

/// The project lines belong to; one without a name when the comment's
/// beginning was out of reach.
fn current(projects: &mut Vec<Project>) -> &mut Project {
    if projects.is_empty() {
        projects.push(Project::default());
    }
    projects.last_mut().expect("never empty here")
}

/// "Ran Plan for project: `a` dir: `b` workspace: `c`", "### 2. project: `a`
/// dir: `b` …", "Ran Plan for dir: `b` workspace: `c`".
fn header(line: &str) -> Option<Project> {
    let rest = line
        .strip_prefix("Ran Plan for ")
        .or_else(|| line.strip_prefix("Ran Apply for "))
        .or_else(|| {
            let rest = line.strip_prefix("### ")?;
            let (n, rest) = rest.split_once(". ")?;
            n.bytes().all(|b| b.is_ascii_digit()).then_some(rest)
        })?;
    let field = |key: &str| -> Option<String> {
        let at = rest.find(&format!("{key}: `"))? + key.len() + 3;
        let end = rest[at..].find('`')? + at;
        Some(rest[at..end].chars().take(MAX_NAME_CHARS).collect())
    };
    let dir = field("dir")?;
    let name = field("project").unwrap_or_else(|| dir.clone());
    Some(Project {
        name,
        dir,
        ..Project::default()
    })
}

/// Terragrunt prints Terraform's lines behind a log prefix:
/// "16:47:01.511 STDOUT terraform: …".
fn without_log_prefix(line: &str) -> &str {
    let Some((head, rest)) = line.split_once(": ") else {
        return line;
    };
    let mut words = head.split(' ');
    let (Some(time), Some(stream), Some(_), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return line;
    };
    let clock = time.len() >= 8 && time.as_bytes()[2] == b':' && time.as_bytes()[5] == b':';
    if clock && (stream == "STDOUT" || stream == "STDERR") {
        rest
    } else {
        line
    }
}

/// "# aws_instance.this[0] will be destroyed" → (Destroy, "aws_instance.this[0]").
fn change_line(line: &str) -> Option<(Op, &str)> {
    let rest = line.strip_prefix("# ")?;
    const OPS: [(&str, Op); 5] = [
        (" will be created", Op::Add),
        (" will be destroyed", Op::Destroy),
        (" will be updated in-place", Op::Change),
        (" must be replaced", Op::Replace),
        (" will be replaced, as requested", Op::Replace),
    ];
    OPS.iter().find_map(|(suffix, op)| {
        let address = rest.strip_suffix(suffix)?;
        (!address.is_empty() && !address.starts_with('(')).then_some((*op, address))
    })
}

/// "module.vpc.module.x[\"a\"].aws_subnet.private[0]" → "aws_subnet".
fn resource_type(address: &str) -> &str {
    let mut rest = address;
    loop {
        if let Some(after) = rest.strip_prefix("module.") {
            // A module's name, with its key in brackets, up to the next dot.
            let mut depth = 0i32;
            let end = after.char_indices().find_map(|(i, c)| {
                match c {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    '.' if depth == 0 => return Some(i),
                    _ => {}
                }
                None
            });
            match end {
                Some(i) => rest = &after[i + 1..],
                None => return address,
            }
        } else if let Some(after) = rest.strip_prefix("data.") {
            rest = after;
        } else {
            return rest.split('.').next().unwrap_or(rest);
        }
    }
}

/// "…currently locked by an unapplied plan from pull #331…" → 331.
fn locked_by(line: &str) -> Option<u64> {
    let at = line.find("locked by")?;
    let rest = &line[at..];
    let hash = rest.find('#')?;
    let digits: String = rest[hash + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok().filter(|n| *n > 0)
}

fn clip(text: &str) -> String {
    let clean: String = text
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    crate::session::truncate(clean.trim(), MAX_ERROR_CHARS)
}

fn is_github_url(url: &str) -> bool {
    url.starts_with("https://github.com/")
        && url.chars().count() <= 512
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// The statuses' run with the comments' detail: the changes grouped, the
/// data at risk, the environment by folder, the error or the lock. The
/// counts are the comments' when they add up to the statuses' (replacements
/// told apart); otherwise the statuses', and the list is partial.
pub fn with_detail(mut plan: Plan, detail: Option<&Detail>) -> Plan {
    let Some(detail) = detail.filter(|_| plan.stage != Stage::Planning) else {
        return plan;
    };
    if matches!(plan.stage, Stage::ApplyFailed) {
        plan.error = detail
            .apply
            .iter()
            .flat_map(|s| &s.projects)
            .find_map(|p| p.error.clone());
    }
    let Some(section) = &detail.plan else {
        return plan;
    };
    plan.comment_url = Some(section.url.clone());
    let changed: Vec<&Project> = section
        .projects
        .iter()
        .filter(|p| !p.changes.is_empty())
        .collect();
    let (mut add, mut change, mut replace, mut destroy) = (0u32, 0u32, 0u32, 0u32);
    let mut groups = Vec::new();
    for project in &changed {
        let label = (changed.len() > 1).then(|| project_label(project));
        for ((op, kind), (count, address)) in &project.changes {
            match op {
                Op::Add => add += count,
                Op::Change => change += count,
                Op::Replace => replace += count,
                Op::Destroy => destroy += count,
            }
            groups.push(Group {
                project: label.clone(),
                op: *op,
                kind: kind.clone(),
                count: *count,
                address: address.clone(),
            });
        }
    }
    let adds_up = !section.cut
        && add + replace == plan.add
        && destroy + replace == plan.destroy
        && change == plan.change;
    if adds_up {
        (plan.add, plan.change, plan.replace, plan.destroy) = (add, change, replace, destroy);
        plan.partial = false;
    }
    // The project with the most dangerous change first; in each, the most
    // dangerous, then the most numerous.
    let mut worst: BTreeMap<Option<String>, Op> = BTreeMap::new();
    for g in &groups {
        let op = worst.entry(g.project.clone()).or_insert(g.op);
        *op = (*op).min(g.op);
    }
    groups.sort_by(|a, b| {
        let key = |g: &Group| {
            (
                worst[&g.project],
                g.project.clone(),
                g.op,
                std::cmp::Reverse(g.count),
                g.kind.clone(),
            )
        };
        key(a).cmp(&key(b))
    });
    if groups.len() > MAX_GROUPS {
        groups.truncate(MAX_GROUPS);
        plan.partial = true;
    }
    plan.data = groups
        .iter()
        .filter(|g| matches!(g.op, Op::Destroy | Op::Replace) && DATA.contains(&g.kind.as_str()))
        .map(|g| g.kind.clone())
        .fold(Vec::new(), |mut kinds, kind| {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
            kinds
        });
    plan.groups = groups;
    plan.drift = section.projects.iter().any(|p| p.drift);
    let envs: Vec<&str> = section
        .projects
        .iter()
        .filter_map(|p| environment(&p.dir))
        .collect();
    if let Some(env) = envs.iter().find(|e| is_production(e)).or(envs.first()) {
        plan.env = Some((*env).to_owned());
        plan.production = envs.iter().any(|e| is_production(e));
    }
    if matches!(plan.stage, Stage::PlanFailed) {
        let failed: Vec<&Project> = section
            .projects
            .iter()
            .filter(|p| p.error.is_some() || p.locked_by.is_some())
            .collect();
        plan.locked_by = failed.iter().find_map(|p| p.locked_by);
        plan.error = failed.iter().find_map(|p| p.error.clone());
        if !failed.is_empty() && failed.iter().all(|p| p.locked_by.is_some()) {
            plan.stage = Stage::Locked;
        }
    }
    plan
}

/// "production · components/compute/ec2/web": the environment and the
/// folders after the region, else the folder.
fn project_label(project: &Project) -> String {
    let parts: Vec<&str> = project.dir.split('/').filter(|p| !p.is_empty()).collect();
    let label = match parts.iter().position(|p| is_region(p)) {
        Some(i) if i > 0 && i + 1 < parts.len() => {
            format!("{} · {}", parts[i - 1], parts[i + 1..].join("/"))
        }
        _ if !project.dir.is_empty() && project.dir != "." => project.dir.clone(),
        _ => project.name.clone(),
    };
    label.chars().take(MAX_NAME_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn status(context: &str, state: &str, description: &str, at: &str) -> Value {
        json!({"context": context, "state": state, "description": description, "createdAt": at})
    }

    #[test]
    fn the_environment_is_the_folder_before_the_region() {
        assert_eq!(
            environment("infraestructure/aws/production/sa-east-1/eks"),
            Some("production")
        );
        assert_eq!(
            environment("infraestructure_aws_pre-prod_sa-east-1_compute_ec2"),
            Some("pre-prod")
        );
        assert_eq!(
            environment("infraestructure/aws/network/us-east-1/vpc"),
            Some("network")
        );
        // A root folder or a sub-folder sharing a production name is no environment.
        assert_eq!(
            environment("infraestructure/aws/pre-prod/sa-east-1/networking/vpc"),
            Some("pre-prod")
        );
        // Without a region, a production folder anywhere.
        assert_eq!(environment("live/prd/vpc"), Some("prd"));
        assert_eq!(environment("live/staging/vpc"), None);
        assert!(
            is_production("Production")
                && !is_production("pre-prod")
                && !is_production("networking")
        );
        assert!(is_region("us-gov-west-1") && !is_region("pre-prod") && !is_region("sa-east-1a"));
    }

    #[test]
    fn the_statuses_tell_the_stage_and_the_counts() {
        assert_eq!(
            from_statuses(&json!([status("ci/test", "SUCCESS", "", "t")])),
            None
        );
        let planning = json!([
            status(
                "atlantis/plan",
                "PENDING",
                "Plan in progress...",
                "2026-11-25T10:00:00Z"
            ),
            status(
                "atlantis/plan: a_production_sa-east-1_x",
                "SUCCESS",
                "Plan: 2 to add, 1 to change, 1 to destroy.",
                "2026-11-25T10:01:00Z"
            ),
            status(
                "atlantis/plan: b_pre-prod_sa-east-1_y",
                "PENDING",
                "Plan in progress...",
                "2026-11-25T10:01:01Z"
            ),
        ]);
        let plan = from_statuses(&planning).unwrap();
        assert_eq!(
            (
                plan.stage,
                plan.projects,
                plan.add,
                plan.change,
                plan.destroy
            ),
            (Stage::Planning, 1, 2, 1, 1)
        );
        assert_eq!(
            (plan.env.as_deref(), plan.production, plan.partial),
            (Some("production"), true, true)
        );
        let planned = json!([
            status(
                "atlantis/plan",
                "SUCCESS",
                "2/2 projects planned successfully.",
                "2026-11-25T10:02:00Z"
            ),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 1 to import, 3 to add, 0 to change, 0 to destroy.",
                "t"
            ),
            status(
                "atlantis/plan: b",
                "SUCCESS",
                "No changes. Your infrastructure matches the configuration.",
                "t"
            ),
            status("atlantis/plan: c", "SUCCESS", "", "t"),
        ]);
        let plan = from_statuses(&planned).unwrap();
        assert_eq!(
            (plan.stage, plan.projects, plan.unchanged, plan.add),
            (Stage::Planned, 3, 2, 3)
        );
        assert_eq!(plan.at, "2026-11-25T10:02:00Z");
        let failed = json!([
            status(
                "atlantis/plan",
                "FAILURE",
                "1/2 projects planned successfully.",
                "t"
            ),
            status("atlantis/plan: a", "FAILURE", "Plan failed.", "t")
        ]);
        assert_eq!(from_statuses(&failed).unwrap().stage, Stage::PlanFailed);
    }

    #[test]
    fn an_apply_after_the_plan_sets_the_stage_and_a_plan_run_again_wins() {
        let run = |apply: &str, apply_at: &str, plan_at: &str| {
            from_statuses(&json!([
                status("atlantis/plan", "SUCCESS", "", plan_at),
                status("atlantis/apply", apply, "", apply_at),
            ]))
            .unwrap()
            .stage
        };
        assert_eq!(
            run("PENDING", "2026-11-25T10:05:00Z", "2026-11-25T10:00:00Z"),
            Stage::Applying
        );
        assert_eq!(
            run("SUCCESS", "2026-11-25T10:05:00Z", "2026-11-25T10:00:00Z"),
            Stage::Applied
        );
        assert_eq!(
            run("FAILURE", "2026-11-25T10:05:00Z", "2026-11-25T10:00:00Z"),
            Stage::ApplyFailed
        );
        assert_eq!(
            run("FAILURE", "2026-11-25T10:05:00Z", "2026-11-25T10:09:00Z"),
            Stage::Planned
        );
    }

    const SINGLE: &str = "Ran Plan for project: `web` dir: `infraestructure/aws/pre-prod/sa-east-1/compute/web` workspace: `default`

<details><summary>Show Output</summary>

```diff
16:47:01.511 STDOUT terraform: Note: Objects have changed outside of Terraform
16:47:01.512 STDOUT terraform:   # aws_instance.this[0] has changed
16:47:01.600 STDOUT terraform:   # aws_instance.this[0] will be destroyed
16:47:01.601 STDOUT terraform:   # aws_security_group.this[0] will be destroyed
16:47:01.602 STDOUT terraform:   # module.sg.aws_vpc_security_group_ingress_rule.this[\"rdp a.b\"] will be destroyed
16:47:01.603 STDOUT terraform:   # module.sg.aws_vpc_security_group_ingress_rule.this[\"ssh\"] will be destroyed
16:47:01.604 STDOUT terraform:   # data.aws_iam_policy_document.x will be read during apply
16:47:01.605 STDOUT terraform: Plan: 0 to add, 0 to change, 4 to destroy.
```
</details>

Plan: 0 to add, 0 to change, 4 to destroy.";

    #[test]
    fn a_single_project_plan_reads_its_changes_and_drift() {
        let detail = read_comments(&[
            ("lgtm", "https://github.com/a/b/pull/1#c0"),
            (SINGLE, "https://github.com/a/b/pull/1#c1"),
        ]);
        let plan = detail.plan.unwrap();
        assert_eq!(
            (plan.url.as_str(), plan.cut, plan.projects.len()),
            ("https://github.com/a/b/pull/1#c1", false, 1)
        );
        let project = &plan.projects[0];
        assert_eq!(
            (project.name.as_str(), project.drift, project.no_changes),
            ("web", true, false)
        );
        assert_eq!(
            project.changes[&(
                Op::Destroy,
                "aws_vpc_security_group_ingress_rule".to_owned()
            )],
            (
                2,
                "module.sg.aws_vpc_security_group_ingress_rule.this[\"rdp a.b\"]".to_owned()
            )
        );
        assert_eq!(project.changes.len(), 3);
        assert!(detail.apply.is_none());
    }

    #[test]
    fn several_projects_continued_comments_and_a_failed_apply_are_read() {
        let first = "Ran Plan for 2 projects:\n\n1. project: `a` dir: `x/production/sa-east-1/db` workspace: `default`\n1. project: `b` dir: `x/pre-prod/sa-east-1/app` workspace: `default`\n\n---\n### 1. project: `a` dir: `x/production/sa-east-1/db` workspace: `default`\n<details><summary>Show Output</summary>\n\n```diff\n10:00:00.000 STDOUT terraform:   # aws_db_instance.orders must be replaced\n```\n</details>\n**Warning**: Output length greater than max comment size. Continued in next comment.\n";
        let second = "Continued plan output from previous comment.\n<details><summary>Show Output</summary>\n\n```diff\n10:00:00.000 STDOUT terraform:   # aws_db_parameter_group.orders will be updated in-place\n```\n</details>\n\nPlan: 1 to add, 1 to change, 1 to destroy.\n\n---\n### 2. project: `b` dir: `x/pre-prod/sa-east-1/app` workspace: `default`\n```diff\nNo changes. Your infrastructure matches the configuration.\n```\n---\n### Plan Summary\n";
        let apply = "Ran Apply for project: `a` dir: `x/production/sa-east-1/db` workspace: `default`\n\n**Apply Error**\n<details><summary>Show Output</summary>\n\n```\nrunning terragrunt\n10:00:00.000 STDERR terraform: │ Error: creating RDS DB Instance: quota exceeded\n```\n</details>";
        let detail = read_comments(&[
            (first, "https://github.com/a/b/pull/2#c1"),
            (second, "https://github.com/a/b/pull/2#c2"),
            (apply, "https://github.com/a/b/pull/2#c3"),
        ]);
        let plan = detail.plan.unwrap();
        assert_eq!(plan.projects.len(), 2);
        assert_eq!(plan.projects[0].changes.len(), 2);
        assert!(plan.projects[1].no_changes);
        let apply = detail.apply.unwrap();
        assert_eq!(
            apply.projects[0].error.as_deref(),
            Some("creating RDS DB Instance: quota exceeded")
        );
        // Only the end of a long plan in reach: still read, marked cut.
        let detail = read_comments(&[(second, "https://github.com/a/b/pull/2#c2")]);
        assert!(detail.plan.unwrap().cut);
    }

    #[test]
    fn the_detail_tells_replacements_apart_groups_by_project_and_flags_data_and_production() {
        let statuses = json!([
            status(
                "atlantis/plan",
                "SUCCESS",
                "2/2 projects planned successfully.",
                "t"
            ),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 1 to add, 1 to change, 1 to destroy.",
                "t"
            ),
            status(
                "atlantis/plan: b",
                "SUCCESS",
                "Plan: 2 to add, 0 to change, 0 to destroy.",
                "t"
            ),
        ]);
        let text = "Ran Plan for 2 projects:\n### 1. project: `a` dir: `x/production/sa-east-1/data/rds` workspace: `default`\n```diff\n  # aws_db_instance.orders must be replaced\n  # aws_db_parameter_group.orders will be updated in-place\n```\n### 2. project: `b` dir: `x/pre-prod/sa-east-1/app` workspace: `default`\n```diff\n  # aws_instance.a will be created\n  # aws_instance.b will be created\n```\n";
        let detail = read_comments(&[(text, "https://github.com/a/b/pull/3#c1")]);
        let plan = with_detail(from_statuses(&statuses).unwrap(), Some(&detail));
        assert_eq!(
            (
                plan.add,
                plan.change,
                plan.replace,
                plan.destroy,
                plan.partial
            ),
            (2, 1, 1, 0, false)
        );
        assert_eq!(
            (plan.env.as_deref(), plan.production),
            (Some("production"), true)
        );
        assert_eq!(plan.data, ["aws_db_instance"]);
        assert_eq!(
            plan.groups[0].project.as_deref(),
            Some("production · data/rds")
        );
        assert_eq!(
            plan.groups.last().unwrap().project.as_deref(),
            Some("pre-prod · app")
        );
        let rds = plan
            .groups
            .iter()
            .find(|g| g.kind == "aws_db_instance")
            .unwrap();
        assert_eq!(
            (rds.project.as_deref(), rds.op, rds.count),
            (Some("production · data/rds"), Op::Replace, 1)
        );
        assert_eq!(
            plan.comment_url.as_deref(),
            Some("https://github.com/a/b/pull/3#c1")
        );
        // Counts that do not add up (an older comment): the statuses' stand.
        let stale = json!([
            status("atlantis/plan", "SUCCESS", "", "t"),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 5 to add, 0 to change, 0 to destroy.",
                "t"
            )
        ]);
        let plan = with_detail(from_statuses(&stale).unwrap(), Some(&detail));
        assert_eq!((plan.add, plan.replace, plan.partial), (5, 0, true));
    }

    #[test]
    fn a_plan_failed_on_a_lock_waits_and_another_failure_says_why() {
        let statuses = json!([
            status(
                "atlantis/plan",
                "FAILURE",
                "0/1 projects planned successfully.",
                "t"
            ),
            status("atlantis/plan: a", "FAILURE", "Plan failed.", "t")
        ]);
        let locked = "Ran Plan for dir: `live/prd/dns` workspace: `default`\n\n**Plan Error**\n```\nThis project is currently locked by an unapplied plan from pull #331. To continue, delete the lock from #331 or apply that plan and merge the pull request.\n```";
        let plan = with_detail(
            from_statuses(&statuses).unwrap(),
            Some(&read_comments(&[(
                locked,
                "https://github.com/a/b/pull/4#c",
            )])),
        );
        assert_eq!((plan.stage, plan.locked_by), (Stage::Locked, Some(331)));
        let failed = "Ran Plan for dir: `live/prd/dns` workspace: `default`\n\n**Plan Failed**: Can't plan your project unless you apply its dependencies";
        let plan = with_detail(
            from_statuses(&statuses).unwrap(),
            Some(&read_comments(&[(
                failed,
                "https://github.com/a/b/pull/4#c",
            )])),
        );
        assert_eq!(plan.stage, Stage::PlanFailed);
        assert_eq!(
            plan.error.as_deref(),
            Some("Can't plan your project unless you apply its dependencies")
        );
    }

    #[test]
    fn odd_lines_are_left_alone() {
        assert_eq!(
            change_line("# (because aws_x.y is not in configuration) will be destroyed"),
            None
        );
        assert_eq!(change_line("+ #!/bin/bash will be created"), None);
        assert_eq!(
            resource_type("module.a[\"x.y\"].module.b.aws_s3_bucket.logs"),
            "aws_s3_bucket"
        );
        assert_eq!(resource_type("data.aws_ami.ubuntu"), "aws_ami");
        assert_eq!(
            without_log_prefix("10:00:00.000 STDOUT terraform:   # x will be created"),
            "  # x will be created"
        );
        assert_eq!(
            without_log_prefix("Note: Objects have changed"),
            "Note: Objects have changed"
        );
        assert_eq!(
            locked_by("locked by an unapplied plan from pull #12."),
            Some(12)
        );
        assert_eq!(
            plan_counts("Plan: 1 to add, 2 to change, 3 to destroy."),
            Some((1, 2, 3))
        );
        assert_eq!(plan_counts("Plan: lots"), None);
        assert!(read_comments(&[(
            "Ran Plan for dir: `a` workspace: `b`",
            "javascript:alert(1)"
        )])
        .plan
        .is_none());
    }
}
