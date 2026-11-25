/**
 * Atlantis plans on the island (#161): the phrase of a plan, its summary
 * when its line is opened, and the runs still going. The facts come from
 * `cortex_core::atlantis`; only the words are made here.
 */

import { button, el } from "../shared/dom";
import type { GithubNotice, Plan, PlanOp, PullRequest } from "../shared/ipc";

const OPS: Record<PlanOp, { sign: string; verb: string; done: string; to: string }> = {
  destroy: { sign: "−", verb: "destroys", done: "destroyed", to: "to destroy" },
  replace: { sign: "±", verb: "replaces", done: "replaced", to: "to replace" },
  change: { sign: "~", verb: "changes", done: "changed", to: "to change" },
  add: { sign: "+", verb: "creates", done: "added", to: "to add" },
};
/** The most dangerous first, as the sentence tells it; the chips read the other way. */
const DANGER: PlanOp[] = ["destroy", "replace", "change", "add"];
const CHIPS: PlanOp[] = ["add", "change", "replace", "destroy"];

/** The commonest AWS types by a name people say; any other drops its provider prefix. */
const KINDS: Record<string, string> = {
  aws_apigatewayv2_integration: "API integration",
  aws_apigatewayv2_route: "API route",
  aws_apigatewayv2_stage: "API stage",
  aws_autoscaling_group: "Auto Scaling group",
  aws_cloudwatch_log_group: "log group",
  aws_cloudwatch_metric_alarm: "alarm",
  aws_cognito_user_pool: "Cognito user pool",
  aws_cognito_user_pool_client: "Cognito app client",
  aws_db_instance: "RDS database",
  aws_db_parameter_group: "parameter group",
  aws_dynamodb_table: "DynamoDB table",
  aws_ebs_volume: "EBS volume",
  aws_ecr_repository: "ECR repository",
  aws_efs_file_system: "EFS file system",
  aws_eks_addon: "EKS add-on",
  aws_eks_cluster: "EKS cluster",
  aws_eks_node_group: "EKS node group",
  aws_elasticache_replication_group: "ElastiCache group",
  aws_iam_instance_profile: "instance profile",
  aws_iam_policy: "IAM policy",
  aws_iam_role: "IAM role",
  aws_iam_role_policy: "role policy",
  aws_iam_role_policy_attachment: "policy attachment",
  aws_iam_user: "IAM user",
  aws_instance: "EC2 instance",
  aws_kms_alias: "KMS alias",
  aws_kms_key: "KMS key",
  aws_lambda_function: "Lambda function",
  aws_launch_template: "launch template",
  aws_lb: "load balancer",
  aws_lb_listener: "listener",
  aws_lb_listener_rule: "listener rule",
  aws_lb_target_group: "target group",
  aws_network_acl_rule: "network ACL rule",
  aws_opensearch_domain: "OpenSearch domain",
  aws_rds_cluster: "RDS cluster",
  aws_route: "route",
  aws_route53_record: "DNS record",
  aws_route53_zone: "DNS zone",
  aws_route_table: "route table",
  aws_s3_bucket: "S3 bucket",
  aws_secretsmanager_secret: "secret",
  aws_security_group: "security group",
  aws_security_group_rule: "security group rule",
  aws_sns_topic: "SNS topic",
  aws_sqs_queue: "SQS queue",
  aws_subnet: "subnet",
  aws_vpc: "VPC",
  aws_vpc_security_group_egress_rule: "egress rule",
  aws_vpc_security_group_ingress_rule: "ingress rule",
  aws_wafv2_ip_set: "WAF IP set",
};

function plural(name: string): string {
  if (/(s|x|ch|sh)$/.test(name)) return `${name}es`;
  if (/[^aeiou]y$/.test(name)) return `${name.slice(0, -1)}ies`;
  return `${name}s`;
}

/** "EC2 instance", "3 EC2 instances": the type a person would say. */
export function kindName(kind: string, n = 1): string {
  const name = KINDS[kind] ?? kind.replace(/^aws_/, "").replace(/_/g, " ");
  return n === 1 ? name : plural(name);
}

const many = (n: number, one: string): string => `${n} ${n === 1 ? one : plural(one)}`;
const both = (parts: string[]): string => (parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}` : (parts[0] ?? ""));

/** The phrase of an Atlantis notice: what the island says for a few seconds. */
export function planSay(kind: GithubNotice["kind"], plan: Plan | undefined): string {
  const counts = (word: "to" | "done", ops: PlanOp[]): string =>
    ops.filter((op) => plan?.[op]).map((op) => `${plan?.[op]} ${word === "to" ? OPS[op].to : OPS[op].done}`).join(", ");
  switch (kind) {
    case "plan_ready":
      return `Plan ready: ${counts("to", ["add", "change"])}`;
    case "plan_destroys": {
      const gone = (plan?.destroy ?? 0) + (plan?.replace ?? 0);
      return `The plan ${plan?.destroy ? "destroys" : "replaces"} ${many(gone, "resource")}`;
    }
    case "plan_failed":
      return "The plan failed";
    case "applied":
      return `Applied: ${counts("done", CHIPS) || "no changes"}`;
    case "apply_failed":
      return "The apply failed";
    default:
      return "";
  }
}

/** One sentence: what the plan does, the most dangerous first, and where. */
export function sentence(plan: Plan): string {
  if (plan.stage === "plan_failed" || plan.stage === "apply_failed") {
    return plan.error ?? `The ${plan.stage === "plan_failed" ? "plan" : "apply"} failed; the reason is in the comment.`;
  }
  if (plan.stage === "locked") {
    return `Waits for ${plan.locked_by ? `#${plan.locked_by}` : "another pull request"}: apply or unlock it there, then plan again.`;
  }
  const kinds = new Map<PlanOp, Map<string, number>>();
  for (const group of plan.groups) {
    const byKind = kinds.get(group.op) ?? new Map<string, number>();
    byKind.set(group.kind, (byKind.get(group.kind) ?? 0) + group.count);
    kinds.set(group.op, byKind);
  }
  const pieces = DANGER.filter((op) => plan[op] > 0).map((op) => {
    const top = [...(kinds.get(op) ?? [])].sort((a, b) => b[1] - a[1]);
    if (plan.partial || top.length === 0) return `${OPS[op].verb} ${many(plan[op], "resource")}`;
    const shown = top.slice(0, 2);
    const rest = plan[op] - shown.reduce((sum, [, n]) => sum + n, 0);
    const named = shown.map(([kind, n]) => `${n} ${kindName(kind, n)}`);
    return `${OPS[op].verb} ${rest > 0 ? `${named.join(", ")} and ${rest} more` : both(named)}`;
  });
  if (pieces.length === 0) return "No changes.";
  const text = pieces.join("; ");
  const where =
    plan.projects > 1
      ? `, across ${plan.projects} projects${plan.unchanged ? ` (${plan.unchanged} without changes)` : ""}`
      : plan.env
        ? `, in ${plan.env}`
        : "";
  return `${text[0]?.toUpperCase()}${text.slice(1)}${where}.`;
}

/** What deserves care, the most serious first: lost data, production, drift. */
export function alerts(plan: Plan): { hot: boolean; text: string }[] {
  const out: { hot: boolean; text: string }[] = [];
  if (plan.data.length > 0) {
    const what = both(plan.data.map((kind) => `the ${kindName(kind)}`));
    out.push({ hot: true, text: `${plan.production ? "In production, data at risk" : "Data at risk"}: ${what}.` });
  } else if (plan.production && plan.destroy + plan.replace > 0) {
    out.push({ hot: true, text: "In production: something is destroyed or replaced." });
  }
  if (plan.drift) out.push({ hot: false, text: "Changed outside Terraform: the apply undoes it." });
  if (plan.partial && plan.stage !== "plan_failed" && plan.stage !== "locked") {
    out.push({ hot: false, text: plan.groups.length > 0 ? "A long plan: the list shows part of it; the counts are complete." : "The counts so far; the detail is on GitHub." });
  }
  return out;
}

/** "+3 ~1": the counts that are not zero, on a notice's line. */
export function planChips(plan: Plan): HTMLElement {
  const chips = el("span", "plan-chips plan-chips--mini");
  for (const op of CHIPS) if (plan[op] > 0) chips.append(el("span", `plan-chip plan-chip--${op} on`, `${OPS[op].sign}${plan[op]}`));
  return chips;
}

/** The summary under an opened line: counts, sentence, alerts, the list, and links. */
export function planCard(plan: Plan, where: string, open: (url: string) => void, prUrl: string): HTMLElement {
  const card = el("div", "plan-card");
  const head = el("div", "plan-where");
  if (plan.env) head.append(el("span", `plan-env${plan.production ? " plan-env--prod" : ""}`, plan.env));
  head.append(el("span", undefined, where));
  card.append(head);
  if (CHIPS.some((op) => plan[op] > 0)) {
    const chips = el("div", "plan-chips");
    for (const op of CHIPS) chips.append(el("span", `plan-chip plan-chip--${op}${plan[op] > 0 ? " on" : ""}`, `${OPS[op].sign}${plan[op]}`));
    card.append(chips);
  }
  card.append(el("div", "plan-sum", sentence(plan)));
  for (const alert of alerts(plan)) card.append(el("div", `plan-alert${alert.hot ? " plan-alert--hot" : ""}`, alert.text));
  if (plan.groups.length > 0) {
    const list = el("ul", "plan-groups");
    list.dataset.scroll = "plan";
    let project: string | null = null;
    for (const group of plan.groups) {
      if (group.project && group.project !== project) list.append(el("li", "plan-project", group.project));
      project = group.project;
      const item = el("li");
      item.title = group.address;
      item.append(el("span", `plan-op plan-op--${group.op}`, OPS[group.op].sign), el("span", "plan-what", group.count > 1 ? `${kindName(group.kind, group.count)} ×${group.count}` : group.address));
      list.append(item);
    }
    if (plan.projects > 1 && plan.unchanged > 0) list.append(el("li", "plan-project", `${many(plan.unchanged, "project")} without changes`));
    card.append(list);
  }
  const actions = el("div", "plan-actions");
  actions.append(button("btn-neutral", "Open PR", () => open(prUrl)));
  if (plan.comment_url) {
    const url = plan.comment_url;
    actions.append(button("btn-neutral", "Full plan", () => open(url)));
  }
  card.append(actions);
  return card;
}

/** A run still going, or waiting on a lock: a gray line, no sound. */
export interface PlanRun {
  key: string;
  say: string;
  detail: string;
  url: string;
  going: boolean;
}

/** The pull requests' runs still going, each once. */
export function planRuns(pulls: PullRequest[]): PlanRun[] {
  const runs = new Map<string, PlanRun>();
  for (const pull of pulls) {
    const plan = pull.plan;
    if (!plan || runs.has(pull.url)) continue;
    const detail = `${pull.repo.split("/").at(-1) ?? pull.repo} #${pull.number}${plan.env ? ` · ${plan.env}` : ""}`;
    const run = { key: `run:${pull.url}`, detail, url: pull.url };
    if (plan.stage === "planning") runs.set(pull.url, { ...run, say: plan.projects > 0 ? `Planning… ${plan.projects} done` : "Planning…", going: true });
    else if (plan.stage === "applying") runs.set(pull.url, { ...run, say: "Applying…", going: true });
    else if (plan.stage === "locked") runs.set(pull.url, { ...run, say: plan.locked_by ? `Locked by #${plan.locked_by}` : "Locked by another pull request", going: false });
  }
  return [...runs.values()];
}
