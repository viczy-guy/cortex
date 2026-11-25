//! Atlantis on pull requests (#161): what the latest plan adds, changes,
//! replaces and destroys. The commit statuses tell where the run stands and
//! the counts, at every check; Atlantis's own plan comment tells the detail,
//! read once the pull request changes. Everything here only reads what came
//! back.

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
const MAX_PROJECTS: usize = 200;
/// Groups kept from one comment; what destroys or replaces gets room past
/// it, so the data at risk is never the part left out.
const MAX_GROUPS: usize = 300;
const MAX_DANGER_GROUPS: usize = 400;
/// Groups the island lists.
const SHOWN_GROUPS: usize = 60;
/// A comment belongs to a run when posted this long before its last status
/// at most (real plans: 0 to 3 s), or this long after: a plan run again
/// takes longer than that, so its predecessor's comment is never taken.
const BEFORE_RUN_SECS: i64 = 30;
const AFTER_RUN_SECS: i64 = 10;

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

/// What deserves care in a plan, the most serious first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alert {
    /// Data or keys destroyed or replaced: the types are in `data`.
    Data,
    /// Something destroyed or replaced in production.
    Production,
    /// Objects changed outside Terraform: the apply undoes it.
    Drift,
    /// The list shows part of the plan; the counts are complete.
    Partial,
    /// Only the statuses' counts so far.
    NoDetail,
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
    pub alerts: Vec<Alert>,
    pub groups: Vec<Group>,
    /// The list shows part of the plan, or none of it.
    pub partial: bool,
    pub error: Option<String>,
    pub locked_by: Option<u64>,
    /// The plan's comment on GitHub.
    pub comment_url: Option<String>,
    /// Projects whose plan failed, by their statuses.
    #[serde(skip)]
    pub failed: u32,
    /// A status that is not Atlantis's fails too (another CI).
    #[serde(skip)]
    pub checks_failed: bool,
    #[serde(skip)]
    pub drift: bool,
    /// When this run's stage began, as GitHub writes it: a new run, a new notice.
    #[serde(skip)]
    pub at: String,
    /// When its plan ran: a comment is only ever read as this plan's when
    /// posted then.
    #[serde(skip)]
    pub planned_at: String,
    /// The account that wrote the plan's status: only its comments are read.
    #[serde(skip)]
    pub author: String,
}

/// The run as the statuses of the pull request's last commit tell it;
/// `None` without Atlantis.
pub fn from_statuses(contexts: &Value) -> Option<Plan> {
    let mut plan_run: Option<(&str, &str, &str)> = None;
    let mut apply_run: Option<(&str, &str, &str)> = None;
    let mut project_applies: Vec<&str> = Vec::new();
    let (mut add, mut change, mut destroy) = (0u32, 0u32, 0u32);
    let (mut projects, mut unchanged, mut failed) = (0u32, 0u32, 0u32);
    let mut checks_failed = false;
    let mut envs = Vec::new();
    for c in contexts.as_array().into_iter().flatten() {
        let (Some(name), Some(state)) = (c["context"].as_str(), c["state"].as_str()) else {
            continue;
        };
        let at = c["createdAt"]
            .as_str()
            .filter(|t| t.len() <= 25)
            .unwrap_or("");
        let description = c["description"].as_str().unwrap_or("");
        if name == "atlantis/plan" {
            let author = c["creator"]["login"]
                .as_str()
                .filter(|l| l.len() <= 64)
                .unwrap_or("");
            plan_run = Some((state, at, author));
        } else if name == "atlantis/apply" {
            apply_run = Some((state, at, description));
        } else if name.starts_with("atlantis/apply: ") {
            project_applies.push(at);
        } else if let Some(project) = name.strip_prefix("atlantis/plan: ") {
            if project.chars().count() > MAX_NAME_CHARS {
                continue;
            }
            if let Some(env) = environment(project) {
                envs.push(env.to_owned());
            }
            match state {
                "SUCCESS" => {
                    projects = projects.saturating_add(1);
                    match plan_counts(description) {
                        Some((a, ch, d)) if a > 0 || ch > 0 || d > 0 => {
                            add = add.saturating_add(a);
                            change = change.saturating_add(ch);
                            destroy = destroy.saturating_add(d);
                        }
                        _ => unchanged = unchanged.saturating_add(1),
                    }
                }
                "FAILURE" | "ERROR" => {
                    projects = projects.saturating_add(1);
                    failed = failed.saturating_add(1);
                }
                _ => {}
            }
        } else if !name.starts_with("atlantis") && matches!(state, "FAILURE" | "ERROR") {
            checks_failed = true;
        }
    }
    let (plan_state, plan_at, author) = plan_run?;
    let planned = match plan_state {
        "PENDING" | "EXPECTED" => Stage::Planning,
        "SUCCESS" if failed == 0 => Stage::Planned,
        _ => Stage::PlanFailed,
    };
    // An apply counts once it came after the latest plan. Atlantis also
    // marks it done when there was nothing to apply, with no project of its
    // own; and it stays pending, done, when only some projects were applied.
    let (stage, at) = match apply_run {
        Some((state, at, description)) if at > plan_at => {
            let ran = project_applies.iter().any(|p| *p > plan_at);
            match state {
                "PENDING" | "EXPECTED"
                    if description.to_ascii_lowercase().contains("in progress") =>
                {
                    (Stage::Applying, at)
                }
                "SUCCESS" if ran => (Stage::Applied, at),
                "FAILURE" | "ERROR" => (Stage::ApplyFailed, at),
                _ => (planned, plan_at),
            }
        }
        _ => (planned, plan_at),
    };
    let production = envs.iter().any(|e| is_production(e));
    let env = envs
        .iter()
        .find(|e| is_production(e))
        .or(envs.first())
        .cloned();
    let mut plan = Plan {
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
        alerts: Vec::new(),
        groups: Vec::new(),
        partial: add > 0 || change > 0 || destroy > 0,
        error: None,
        locked_by: None,
        comment_url: None,
        failed,
        checks_failed,
        drift: false,
        at: at.into(),
        planned_at: plan_at.into(),
        author: author.into(),
    };
    plan.alerts = alerts(&plan);
    Some(plan)
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
    let Some((number, letters)) = words.split_last() else {
        return false;
    };
    words.len() >= 3
        && words[0].len() == 2
        && letters
            .iter()
            .all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase()))
        && !number.is_empty()
        && number.bytes().all(|b| b.is_ascii_digit())
}

/// One comment of a pull request, by Atlantis: its text, link and time.
#[derive(Debug, Clone, Copy)]
pub struct Comment<'a> {
    pub body: &'a str,
    pub url: &'a str,
    pub created_at: &'a str,
}

/// One project of a plan or an apply comment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub dir: String,
    /// (op, resource type) → (count, first address), as far as kept.
    pub changes: BTreeMap<(Op, String), (u32, String)>,
    /// Every resource line, kept or not, by op (destroy, replace, change, add).
    pub ops: [u32; 4],
    pub no_changes: bool,
    pub drift: bool,
    pub error: Option<String>,
    pub locked_by: Option<u64>,
}

/// The latest plan, or apply, out of a pull request's last comments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    pub url: String,
    /// When its first part was posted.
    pub created_at: String,
    pub projects: Vec<Project>,
    /// Its beginning was out of the comments read.
    pub cut: bool,
    /// Some of its changes were counted but not kept in the groups.
    pub capped: bool,
    /// Every resource type with data destroyed or replaced, groups kept or not.
    pub data: std::collections::BTreeSet<String>,
}

/// The pull request's latest plan and apply comments, as far as they were read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    pub plan: Option<Section>,
    pub apply: Option<Section>,
}

/// Read Atlantis's comments of a pull request, oldest first.
pub fn read_comments(comments: &[Comment]) -> Detail {
    Detail {
        plan: section(comments, "Ran Plan for", "Continued plan output"),
        apply: section(comments, "Ran Apply for", "Continued apply output"),
    }
}

fn section(comments: &[Comment], start: &str, continued: &str) -> Option<Section> {
    let begins = |body: &str| body.trim_start().starts_with(start);
    let continues = |body: &str| {
        let body = body.trim_start();
        body.starts_with(continued) || body.starts_with("Continued from previous comment")
    };
    let (from, cut) = match comments.iter().rposition(|c| begins(c.body)) {
        Some(i) => (i, false),
        // The beginning went out of reach: what is left still counts.
        None => (comments.iter().position(|c| continues(c.body))?, true),
    };
    let parts: Vec<&Comment> = comments[from..]
        .iter()
        .enumerate()
        .take_while(|(i, c)| *i == 0 || continues(c.body))
        .map(|(_, c)| c)
        .collect();
    let first = parts
        .first()
        .filter(|c| crate::github::is_github_url(c.url))?;
    let mut read = Section {
        url: first.url.into(),
        created_at: first.created_at.chars().take(25).collect(),
        projects: Vec::new(),
        cut,
        capped: false,
        data: Default::default(),
    };
    let mut groups = 0usize;
    'parts: for comment in parts {
        // Each comment is a document of its own: its fences open and close in it.
        let mut fenced = false;
        // After "**Plan Error**": its block's first "Error: …" line, else its first line.
        let mut error_block = false;
        let mut plain_error = false;
        for line in comment.body.lines() {
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
                    if read.projects.len() < MAX_PROJECTS {
                        read.projects.push(project);
                    } else {
                        read.capped = true;
                    }
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
                let kind: String = resource_type(address)
                    .chars()
                    .take(MAX_NAME_CHARS)
                    .collect();
                if matches!(op, Op::Destroy | Op::Replace) && DATA.contains(&kind.as_str()) {
                    read.data.insert(kind.clone());
                }
                let project = current(&mut read.projects);
                project.ops[op as usize] = project.ops[op as usize].saturating_add(1);
                let key = (op, kind);
                if !project.changes.contains_key(&key) {
                    let room = if matches!(op, Op::Destroy | Op::Replace) {
                        MAX_DANGER_GROUPS
                    } else {
                        MAX_GROUPS
                    };
                    if groups >= room {
                        read.capped = true;
                        continue;
                    }
                    groups += 1;
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

/// "module.vpc.module.x[\"a.b]\"].aws_subnet.private[0]" → "aws_subnet".
fn resource_type(address: &str) -> &str {
    let mut rest = address;
    loop {
        if let Some(after) = rest.strip_prefix("module.") {
            // A module's name, with its key in brackets (quoted keys may hold
            // dots and brackets), up to the next dot.
            let (mut depth, mut quoted, mut escaped) = (0i32, false, false);
            let mut end = None;
            for (i, c) in after.char_indices() {
                if quoted {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        quoted = false;
                    }
                    continue;
                }
                match c {
                    '"' => quoted = true,
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    '.' if depth == 0 => {
                        end = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
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

/// GitHub's "2026-11-25T10:00:00Z" in seconds since 1970; `None` for
/// anything else (days from civil: H. Hinnant's algorithm).
fn epoch_secs(time: &str) -> Option<i64> {
    let b = time.as_bytes();
    let shape = b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z';
    if !shape {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let text = time.get(from..to)?;
        text.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| text.parse().ok())?
    };
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// A comment posted at `posted` belongs to the run whose last status came at
/// `run`: Atlantis posts it seconds before; an older run's is minutes away.
fn same_run(posted: &str, run: &str) -> bool {
    match (epoch_secs(posted), epoch_secs(run)) {
        (Some(posted), Some(run)) => (-BEFORE_RUN_SECS..=AFTER_RUN_SECS).contains(&(posted - run)),
        _ => false,
    }
}

/// The statuses' run with the comments' detail: the changes grouped, the
/// data at risk, the environment by folder, the error or the lock. The
/// counts are the comments' when they add up to the statuses' (replacements
/// told apart) or show more going; otherwise the statuses', and the list is
/// partial. Only comments posted for this very run count.
pub fn with_detail(mut plan: Plan, detail: Option<&Detail>) -> Plan {
    let detail = detail.filter(|_| plan.stage != Stage::Planning);
    if plan.stage == Stage::ApplyFailed {
        plan.error = detail
            .and_then(|d| d.apply.as_ref())
            .filter(|s| same_run(&s.created_at, &plan.at))
            .and_then(|s| s.projects.iter().find_map(|p| p.error.clone()));
    }
    let section = detail
        .and_then(|d| d.plan.as_ref())
        .filter(|s| same_run(&s.created_at, &plan.planned_at));
    if let Some(section) = section {
        merge(&mut plan, section);
    }
    plan.alerts = alerts(&plan);
    plan
}

fn merge(plan: &mut Plan, section: &Section) {
    plan.comment_url = Some(section.url.clone());
    let mut ops = [0u64; 4];
    for project in &section.projects {
        for (sum, n) in ops.iter_mut().zip(project.ops) {
            *sum += u64::from(n);
        }
    }
    let [destroy, replace, change, add] = ops;
    let adds_up = !section.cut
        && add + replace == u64::from(plan.add)
        && destroy + replace == u64::from(plan.destroy)
        && change == u64::from(plan.change);
    // The statuses read one "Plan:" line per project: a project that prints
    // several (a run-all) may hide what goes; the comment never does. Part of
    // one only ever raises what goes.
    let more_going = destroy + replace > u64::from(plan.destroy);
    let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    if adds_up || (more_going && !section.cut) {
        (plan.add, plan.change, plan.replace, plan.destroy) =
            (clamp(add), clamp(change), clamp(replace), clamp(destroy));
        plan.partial = section.capped;
    } else if more_going {
        (plan.replace, plan.destroy) = (clamp(replace), clamp(destroy));
    }
    let changed: Vec<&Project> = section
        .projects
        .iter()
        .filter(|p| !p.changes.is_empty())
        .collect();
    let mut groups: Vec<Group> = changed
        .iter()
        .flat_map(|project| {
            let label = (changed.len() > 1).then(|| project_label(project));
            project
                .changes
                .iter()
                .map(move |((op, kind), (count, address))| Group {
                    project: label.clone(),
                    op: *op,
                    kind: kind.clone(),
                    count: *count,
                    address: address.clone(),
                })
        })
        .collect();
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
    plan.data = section.data.iter().cloned().collect();
    if groups.len() > SHOWN_GROUPS {
        groups.truncate(SHOWN_GROUPS);
        plan.partial = true;
    }
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
    if plan.stage == Stage::PlanFailed {
        let locked: Vec<&Project> = section
            .projects
            .iter()
            .filter(|p| p.locked_by.is_some())
            .collect();
        let errors: Vec<&Project> = section
            .projects
            .iter()
            .filter(|p| p.error.is_some() && p.locked_by.is_none())
            .collect();
        plan.locked_by = locked.first().and_then(|p| p.locked_by);
        plan.error = errors
            .first()
            .or(locked.first())
            .and_then(|p| p.error.clone());
        // Only when every failure the statuses count is a lock in sight.
        let whole = !section.cut && !section.capped;
        if whole && plan.failed > 0 && locked.len() >= plan.failed as usize && errors.is_empty() {
            plan.stage = Stage::Locked;
        }
    }
}

/// What deserves care, the most serious first.
fn alerts(plan: &Plan) -> Vec<Alert> {
    let mut out = Vec::new();
    let goes = plan.destroy > 0 || plan.replace > 0;
    if !plan.data.is_empty() {
        out.push(Alert::Data);
    } else if plan.production && goes {
        out.push(Alert::Production);
    }
    if plan.drift {
        out.push(Alert::Drift);
    }
    let failed = matches!(
        plan.stage,
        Stage::PlanFailed | Stage::Locked | Stage::ApplyFailed
    );
    if plan.partial && !failed {
        out.push(if plan.groups.is_empty() {
            Alert::NoDetail
        } else {
            Alert::Partial
        });
    }
    out
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

    const AT: &str = "2026-11-25T10:00:00Z";
    const POSTED: &str = "2026-11-25T09:59:58Z";

    fn status(context: &str, state: &str, description: &str, at: &str) -> Value {
        json!({"context": context, "state": state, "description": description, "createdAt": at, "creator": {"login": "atlantis-acme[bot]"}})
    }

    fn comment<'a>(body: &'a str, url: &'a str) -> Comment<'a> {
        Comment {
            body,
            url,
            created_at: POSTED,
        }
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
        assert!(is_production("Production") && !is_production("pre-prod"));
        assert!(!is_production("networking"));
        assert!(is_region("us-gov-west-1") && !is_region("pre-prod") && !is_region("sa-east-1a"));
    }

    #[test]
    fn the_statuses_tell_the_stage_and_the_counts() {
        assert_eq!(
            from_statuses(&json!([status("ci/test", "SUCCESS", "", "t")])),
            None
        );
        let planning = json!([
            status("atlantis/plan", "PENDING", "Plan in progress...", AT),
            status(
                "atlantis/plan: a_production_sa-east-1_x",
                "SUCCESS",
                "Plan: 2 to add, 1 to change, 1 to destroy.",
                AT
            ),
            status(
                "atlantis/plan: b_pre-prod_sa-east-1_y",
                "PENDING",
                "Plan in progress...",
                AT
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
            (plan.env.as_deref(), plan.production),
            (Some("production"), true)
        );
        let planned = json!([
            status(
                "atlantis/plan",
                "SUCCESS",
                "2/2 projects planned successfully.",
                AT
            ),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 1 to import, 3 to add, 0 to change, 0 to destroy.",
                AT
            ),
            status(
                "atlantis/plan: b",
                "SUCCESS",
                "No changes. Your infrastructure matches the configuration.",
                AT
            ),
            status("atlantis/plan: c", "SUCCESS", "", AT),
            status(
                "atlantis/pre_workflow_hook: graph",
                "FAILURE",
                "failed.",
                AT
            ),
        ]);
        let plan = from_statuses(&planned).unwrap();
        assert_eq!(
            (plan.stage, plan.projects, plan.unchanged, plan.add),
            (Stage::Planned, 3, 2, 3)
        );
        assert_eq!((plan.at.as_str(), plan.checks_failed), (AT, false));
        assert_eq!(plan.alerts, [Alert::NoDetail]);
        let failed = json!([
            status(
                "atlantis/plan",
                "FAILURE",
                "1/2 projects planned successfully.",
                AT
            ),
            status("atlantis/plan: a", "FAILURE", "Plan failed.", AT),
            status("ci/lint", "FAILURE", "", AT),
        ]);
        let plan = from_statuses(&failed).unwrap();
        assert_eq!(
            (plan.stage, plan.failed, plan.checks_failed),
            (Stage::PlanFailed, 1, true)
        );
        // Counts at the limit saturate, never overflow.
        let huge = json!([
            status("atlantis/plan", "SUCCESS", "", AT),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 4294967295 to add, 1 to change, 0 to destroy.",
                AT
            ),
            status(
                "atlantis/plan: b",
                "SUCCESS",
                "Plan: 9 to add, 0 to change, 0 to destroy.",
                AT
            ),
        ]);
        assert_eq!(from_statuses(&huge).unwrap().add, u32::MAX);
    }

    #[test]
    fn an_apply_counts_after_the_plan_with_projects_of_its_own() {
        let run = |apply: &str, description: &str, apply_at: &str, project: Option<&str>| {
            let mut statuses = vec![
                status("atlantis/plan", "SUCCESS", "", AT),
                status("atlantis/apply", apply, description, apply_at),
            ];
            if let Some(at) = project {
                statuses.push(status(
                    "atlantis/apply: a",
                    "SUCCESS",
                    "Apply succeeded.",
                    at,
                ));
            }
            from_statuses(&Value::Array(statuses)).unwrap().stage
        };
        let later = "2026-11-25T10:05:00Z";
        assert_eq!(
            run("PENDING", "Apply in progress...", later, None),
            Stage::Applying
        );
        assert_eq!(
            run(
                "SUCCESS",
                "1/1 projects applied successfully.",
                later,
                Some(later)
            ),
            Stage::Applied
        );
        assert_eq!(run("FAILURE", "", later, Some(later)), Stage::ApplyFailed);
        // Nothing to apply: Atlantis marks it done with no project.
        assert_eq!(
            run("SUCCESS", "0/0 projects applied successfully.", AT, None),
            Stage::Planned
        );
        // Some projects applied, the rest still planned.
        assert_eq!(
            run(
                "PENDING",
                "1/3 projects applied successfully.",
                later,
                Some(later)
            ),
            Stage::Planned
        );
        // A plan run again later wins.
        assert_eq!(
            run(
                "FAILURE",
                "",
                "2026-11-25T09:00:00Z",
                Some("2026-11-25T09:00:00Z")
            ),
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
            comment("lgtm", "https://github.com/a/b/pull/1#c0"),
            comment(SINGLE, "https://github.com/a/b/pull/1#c1"),
        ]);
        let plan = detail.plan.unwrap();
        assert_eq!(
            (
                plan.url.as_str(),
                plan.created_at.as_str(),
                plan.cut,
                plan.projects.len()
            ),
            ("https://github.com/a/b/pull/1#c1", POSTED, false, 1)
        );
        let project = &plan.projects[0];
        assert_eq!(
            (project.name.as_str(), project.drift, project.no_changes),
            ("web", true, false)
        );
        assert_eq!(project.ops, [4, 0, 0, 0]);
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
            comment(first, "https://github.com/a/b/pull/2#c1"),
            comment(second, "https://github.com/a/b/pull/2#c2"),
            comment(apply, "https://github.com/a/b/pull/2#c3"),
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
        let detail = read_comments(&[comment(second, "https://github.com/a/b/pull/2#c2")]);
        assert!(detail.plan.unwrap().cut);
    }

    fn two_projects() -> (Value, Detail) {
        let statuses = json!([
            status(
                "atlantis/plan",
                "SUCCESS",
                "2/2 projects planned successfully.",
                AT
            ),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 1 to add, 1 to change, 1 to destroy.",
                AT
            ),
            status(
                "atlantis/plan: b",
                "SUCCESS",
                "Plan: 2 to add, 0 to change, 0 to destroy.",
                AT
            ),
        ]);
        let text = "Ran Plan for 2 projects:\n### 1. project: `a` dir: `x/production/sa-east-1/data/rds` workspace: `default`\n```diff\n  # aws_db_instance.orders must be replaced\n  # aws_db_parameter_group.orders will be updated in-place\n```\n### 2. project: `b` dir: `x/pre-prod/sa-east-1/app` workspace: `default`\n```diff\n  # aws_instance.a will be created\n  # aws_instance.b will be created\n```\n";
        let detail = read_comments(&[comment(text, "https://github.com/a/b/pull/3#c1")]);
        (statuses, detail)
    }

    #[test]
    fn the_detail_tells_replacements_apart_groups_by_project_and_flags_data_and_production() {
        let (statuses, detail) = two_projects();
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
        assert_eq!(
            (plan.data.as_slice(), plan.alerts.as_slice()),
            (
                ["aws_db_instance".to_owned()].as_slice(),
                [Alert::Data].as_slice()
            )
        );
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
    }

    #[test]
    fn a_comment_of_another_run_or_out_of_step_counts_is_not_taken_for_this_one() {
        let (statuses, detail) = two_projects();
        // Posted for a run minutes before: none of it shows.
        let mut older = detail.clone();
        older.plan.as_mut().unwrap().created_at = "2026-11-25T09:50:00Z".into();
        let plan = with_detail(from_statuses(&statuses).unwrap(), Some(&older));
        assert_eq!(
            (
                plan.groups.len(),
                plan.comment_url.as_deref(),
                plan.alerts.as_slice()
            ),
            (0, None, [Alert::NoDetail].as_slice())
        );
        // Counts that do not add up: the statuses' stand, the list is partial.
        let stale = json!([
            status("atlantis/plan", "SUCCESS", "", AT),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 5 to add, 0 to change, 3 to destroy.",
                AT
            ),
        ]);
        let plan = with_detail(from_statuses(&stale).unwrap(), Some(&detail));
        assert_eq!(
            (plan.add, plan.replace, plan.destroy, plan.partial),
            (5, 0, 3, true)
        );
        // More going in the comment than the statuses say: the comment wins.
        let short = json!([
            status("atlantis/plan", "SUCCESS", "", AT),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 3 to add, 1 to change, 0 to destroy.",
                AT
            ),
        ]);
        let plan = with_detail(from_statuses(&short).unwrap(), Some(&detail));
        assert_eq!((plan.replace, plan.destroy), (1, 0));
    }

    #[test]
    fn a_plan_failed_on_locks_alone_waits_and_another_failure_says_why() {
        let statuses = |failed: usize| {
            let mut all = vec![status("atlantis/plan", "FAILURE", "", AT)];
            for i in 0..failed {
                all.push(status(
                    &format!("atlantis/plan: p{i}"),
                    "FAILURE",
                    "Plan failed.",
                    AT,
                ));
            }
            Value::Array(all)
        };
        let locked = "Ran Plan for dir: `live/prd/dns` workspace: `default`\n\n**Plan Error**\n```\nThis project is currently locked by an unapplied plan from pull #331. To continue, delete the lock from #331 or apply that plan and merge the pull request.\n```";
        let detail = read_comments(&[comment(locked, "https://github.com/a/b/pull/4#c")]);
        let plan = with_detail(from_statuses(&statuses(1)).unwrap(), Some(&detail));
        assert_eq!((plan.stage, plan.locked_by), (Stage::Locked, Some(331)));
        // Two failures, one lock in sight: the other may be a real error.
        let plan = with_detail(from_statuses(&statuses(2)).unwrap(), Some(&detail));
        assert_eq!(plan.stage, Stage::PlanFailed);
        let failed = "Ran Plan for dir: `live/prd/dns` workspace: `default`\n\n**Plan Failed**: Can't plan your project unless you apply its dependencies";
        let detail = read_comments(&[comment(failed, "https://github.com/a/b/pull/4#c")]);
        let plan = with_detail(from_statuses(&statuses(1)).unwrap(), Some(&detail));
        assert_eq!(plan.stage, Stage::PlanFailed);
        assert_eq!(
            plan.error.as_deref(),
            Some("Can't plan your project unless you apply its dependencies")
        );
    }

    #[test]
    fn what_goes_keeps_its_place_when_a_comment_has_too_much() {
        let mut text = String::from("Ran Plan for dir: `a` workspace: `default`\n```diff\n");
        for i in 0..MAX_GROUPS + 5 {
            text.push_str(&format!("  # aws_thing_{i}.x will be created\n"));
        }
        text.push_str("  # aws_s3_bucket.logs will be destroyed\n```\n");
        let detail = read_comments(&[comment(&text, "https://github.com/a/b/pull/5#c")]);
        let section = detail.plan.as_ref().unwrap();
        assert!(section.capped);
        assert_eq!(
            section.projects[0].ops[Op::Add as usize] as usize,
            MAX_GROUPS + 5
        );
        let statuses = json!([
            status("atlantis/plan", "SUCCESS", "", AT),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                &format!(
                    "Plan: {} to add, 0 to change, 1 to destroy.",
                    MAX_GROUPS + 5
                ),
                AT
            ),
        ]);
        let plan = with_detail(from_statuses(&statuses).unwrap(), Some(&detail));
        assert_eq!(
            (plan.destroy, plan.partial, plan.data.as_slice()),
            (1, true, ["aws_s3_bucket".to_owned()].as_slice())
        );
        assert_eq!(plan.groups[0].kind, "aws_s3_bucket");
        assert_eq!(plan.groups.len(), SHOWN_GROUPS);
    }

    #[test]
    fn the_review_pass_cases_hold() {
        // Who Atlantis is comes with the plan's status.
        let one = json!([status("atlantis/plan", "SUCCESS", "", AT)]);
        assert_eq!(from_statuses(&one).unwrap().author, "atlantis-acme[bot]");
        // An apply status in the same second as the plan is no apply.
        let same = json!([
            status("atlantis/plan", "SUCCESS", "", AT),
            status(
                "atlantis/apply",
                "SUCCESS",
                "1/1 projects applied successfully.",
                AT
            ),
            status("atlantis/apply: a", "SUCCESS", "Apply succeeded.", AT),
        ]);
        assert_eq!(from_statuses(&same).unwrap().stage, Stage::Planned);
        // The window: up to 30 s before the run's last status, 10 s after.
        assert!(same_run("2026-11-25T09:59:30Z", AT) && !same_run("2026-11-25T09:59:29Z", AT));
        assert!(same_run("2026-11-25T10:00:10Z", AT) && !same_run("2026-11-25T10:00:11Z", AT));
        // The data at risk survives any cap on the groups.
        let mut text = String::from("Ran Plan for dir: `a` workspace: `default`\n```diff\n");
        for i in 0..MAX_DANGER_GROUPS + 2 {
            text.push_str(&format!("  # aws_thing_{i}.x will be destroyed\n"));
        }
        text.push_str("  # aws_s3_bucket.logs will be destroyed\n```\n");
        let detail = read_comments(&[comment(&text, "https://github.com/a/b/pull/6#c")]);
        let section = detail.plan.as_ref().unwrap();
        assert!(section.capped && section.data.contains("aws_s3_bucket"));
        // A cut comment raises what goes, and a lock needs the whole comment.
        let continued = "Continued plan output from previous comment.\n```diff\n  # aws_db_instance.a will be destroyed\n```\nThis project is currently locked by an unapplied plan from pull #9.";
        let detail = read_comments(&[comment(continued, "https://github.com/a/b/pull/7#c")]);
        let creates = json!([
            status("atlantis/plan", "SUCCESS", "", AT),
            status(
                "atlantis/plan: a",
                "SUCCESS",
                "Plan: 2 to add, 0 to change, 0 to destroy.",
                AT
            ),
        ]);
        let plan = with_detail(from_statuses(&creates).unwrap(), Some(&detail));
        assert_eq!(
            (plan.add, plan.destroy, plan.data.as_slice()),
            (2, 1, ["aws_db_instance".to_owned()].as_slice())
        );
        let failed = json!([
            status("atlantis/plan", "FAILURE", "", AT),
            status("atlantis/plan: a", "FAILURE", "", AT)
        ]);
        assert_eq!(
            with_detail(from_statuses(&failed).unwrap(), Some(&detail)).stage,
            Stage::PlanFailed
        );
    }

    #[test]
    fn odd_lines_and_times_are_left_alone() {
        assert_eq!(
            change_line("# (because aws_x.y is not in configuration) will be destroyed"),
            None
        );
        assert_eq!(change_line("+ #!/bin/bash will be created"), None);
        assert_eq!(
            resource_type("module.a[\"x.y\"].module.b.aws_s3_bucket.logs"),
            "aws_s3_bucket"
        );
        assert_eq!(
            resource_type("module.a[\"x]y.z\"].aws_s3_bucket.logs"),
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
        assert_eq!(epoch_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch_secs(AT), Some(1_795_600_800));
        assert_eq!(epoch_secs("2000-02-29T23:59:59Z"), Some(951_868_799));
        for bad in [
            "",
            "2026-11-25 10:00:00Z",
            "2026-13-25T10:00:00Z",
            "2026-11-25T10:00:+0Z",
        ] {
            assert_eq!(epoch_secs(bad), None, "{bad}");
        }
        assert!(same_run(POSTED, AT) && !same_run("2026-11-25T09:55:00Z", AT) && !same_run("", AT));
        let fake = Comment {
            body: "Ran Plan for dir: `a` workspace: `b`",
            url: "javascript:alert(1)",
            created_at: POSTED,
        };
        assert!(read_comments(&[fake]).plan.is_none());
    }
}
