/**
 * Atlantis plans on the island (#161): the phrase of a plan, its summary
 * when its line is opened, and the runs still going. The facts come from
 * `cortex_core::atlantis`; only the words are made here, in the language
 * shown (#154).
 */

import { button, el } from "../shared/dom";
import { language, t } from "../shared/i18n";
import type { GithubNotice, Plan, PlanOp, PullRequest } from "../shared/ipc";

const SIGNS: Record<PlanOp, string> = { destroy: "−", replace: "±", change: "~", add: "+" };
/** The most dangerous first, as the sentence tells it; the chips read the other way. */
const DANGER: PlanOp[] = ["destroy", "replace", "change", "add"];
const CHIPS: PlanOp[] = ["add", "change", "replace", "destroy"];

/**
 * The commonest AWS types by a name people say (Portuguese keeps the words
 * engineers keep in English); any other drops its provider prefix.
 */
const KINDS: Record<string, { en: string; pt: [string, string] }> = {
  aws_apigatewayv2_integration: { en: "API integration", pt: ["integração de API", "integrações de API"] },
  aws_apigatewayv2_route: { en: "API route", pt: ["rota de API", "rotas de API"] },
  aws_apigatewayv2_stage: { en: "API stage", pt: ["stage de API", "stages de API"] },
  aws_autoscaling_group: { en: "Auto Scaling group", pt: ["Auto Scaling group", "Auto Scaling groups"] },
  aws_cloudwatch_log_group: { en: "log group", pt: ["log group", "log groups"] },
  aws_cloudwatch_metric_alarm: { en: "alarm", pt: ["alarme", "alarmes"] },
  aws_cognito_user_pool: { en: "Cognito user pool", pt: ["user pool do Cognito", "user pools do Cognito"] },
  aws_cognito_user_pool_client: { en: "Cognito app client", pt: ["app client do Cognito", "app clients do Cognito"] },
  aws_db_instance: { en: "RDS database", pt: ["banco RDS", "bancos RDS"] },
  aws_db_parameter_group: { en: "parameter group", pt: ["parameter group", "parameter groups"] },
  aws_dynamodb_table: { en: "DynamoDB table", pt: ["tabela DynamoDB", "tabelas DynamoDB"] },
  aws_ebs_volume: { en: "EBS volume", pt: ["volume EBS", "volumes EBS"] },
  aws_ecr_repository: { en: "ECR repository", pt: ["repositório ECR", "repositórios ECR"] },
  aws_efs_file_system: { en: "EFS file system", pt: ["file system EFS", "file systems EFS"] },
  aws_eks_addon: { en: "EKS add-on", pt: ["add-on do EKS", "add-ons do EKS"] },
  aws_eks_cluster: { en: "EKS cluster", pt: ["cluster EKS", "clusters EKS"] },
  aws_eks_node_group: { en: "EKS node group", pt: ["node group do EKS", "node groups do EKS"] },
  aws_elasticache_replication_group: { en: "ElastiCache group", pt: ["grupo ElastiCache", "grupos ElastiCache"] },
  aws_iam_instance_profile: { en: "instance profile", pt: ["instance profile", "instance profiles"] },
  aws_iam_policy: { en: "IAM policy", pt: ["policy IAM", "policies IAM"] },
  aws_iam_role: { en: "IAM role", pt: ["role IAM", "roles IAM"] },
  aws_iam_role_policy: { en: "role policy", pt: ["policy de role", "policies de role"] },
  aws_iam_role_policy_attachment: { en: "policy attachment", pt: ["policy attachment", "policy attachments"] },
  aws_iam_user: { en: "IAM user", pt: ["usuário IAM", "usuários IAM"] },
  aws_instance: { en: "EC2 instance", pt: ["instância EC2", "instâncias EC2"] },
  aws_kms_alias: { en: "KMS alias", pt: ["alias KMS", "aliases KMS"] },
  aws_kms_key: { en: "KMS key", pt: ["chave KMS", "chaves KMS"] },
  aws_lambda_function: { en: "Lambda function", pt: ["função Lambda", "funções Lambda"] },
  aws_launch_template: { en: "launch template", pt: ["launch template", "launch templates"] },
  aws_lb: { en: "load balancer", pt: ["load balancer", "load balancers"] },
  aws_lb_listener: { en: "listener", pt: ["listener", "listeners"] },
  aws_lb_listener_rule: { en: "listener rule", pt: ["regra de listener", "regras de listener"] },
  aws_lb_target_group: { en: "target group", pt: ["target group", "target groups"] },
  aws_network_acl_rule: { en: "network ACL rule", pt: ["regra de ACL", "regras de ACL"] },
  aws_opensearch_domain: { en: "OpenSearch domain", pt: ["domínio OpenSearch", "domínios OpenSearch"] },
  aws_rds_cluster: { en: "RDS cluster", pt: ["cluster RDS", "clusters RDS"] },
  aws_route: { en: "route", pt: ["rota", "rotas"] },
  aws_route53_record: { en: "DNS record", pt: ["registro DNS", "registros DNS"] },
  aws_route53_zone: { en: "DNS zone", pt: ["zona DNS", "zonas DNS"] },
  aws_route_table: { en: "route table", pt: ["tabela de rotas", "tabelas de rotas"] },
  aws_s3_bucket: { en: "S3 bucket", pt: ["bucket S3", "buckets S3"] },
  aws_secretsmanager_secret: { en: "secret", pt: ["secret", "secrets"] },
  aws_security_group: { en: "security group", pt: ["security group", "security groups"] },
  aws_security_group_rule: { en: "security group rule", pt: ["regra de security group", "regras de security group"] },
  aws_sns_topic: { en: "SNS topic", pt: ["tópico SNS", "tópicos SNS"] },
  aws_sqs_queue: { en: "SQS queue", pt: ["fila SQS", "filas SQS"] },
  aws_subnet: { en: "subnet", pt: ["subnet", "subnets"] },
  aws_vpc: { en: "VPC", pt: ["VPC", "VPCs"] },
  aws_vpc_security_group_egress_rule: { en: "egress rule", pt: ["regra de saída", "regras de saída"] },
  aws_vpc_security_group_ingress_rule: { en: "ingress rule", pt: ["regra de entrada", "regras de entrada"] },
  aws_wafv2_ip_set: { en: "WAF IP set", pt: ["IP set do WAF", "IP sets do WAF"] },
};

function plural(name: string): string {
  if (/(s|x|ch|sh)$/.test(name)) return `${name}es`;
  if (/[^aeiou]y$/.test(name)) return `${name.slice(0, -1)}ies`;
  return `${name}s`;
}

/** "EC2 instance", "3 EC2 instances": the type a person would say. */
export function kindName(kind: string, n = 1): string {
  const known = KINDS[kind];
  if (known && language() === "pt") return known.pt[n === 1 ? 0 : 1];
  // A type not listed keeps its English name, in Portuguese too.
  const name = known?.en ?? kind.replace(/^aws_/, "").replace(/_/g, " ");
  return n === 1 ? name : plural(name);
}

const both = (parts: string[]): string =>
  parts.length > 1 ? `${parts.slice(0, -1).join(", ")} ${t("plan.and")} ${parts.at(-1)}` : (parts[0] ?? "");

/** The phrase of an Atlantis notice: what the island says for a few seconds. */
export function planSay(kind: GithubNotice["kind"], plan: Plan | undefined): string {
  const done = { add: "plan.done.add", change: "plan.done.change", replace: "plan.done.replace", destroy: "plan.done.destroy" } as const;
  switch (kind) {
    case "plan_ready": {
      const parts = [plan?.add ? t("plan.to.add", plan.add) : "", plan?.change ? t("plan.to.change", plan.change) : ""];
      return t("plan.ready", parts.filter(Boolean).join(", "));
    }
    case "plan_destroys": {
      const gone = (plan?.destroy ?? 0) + (plan?.replace ?? 0);
      return plan?.destroy ? t("plan.destroys", gone) : t("plan.replaces", gone);
    }
    case "plan_failed":
      return t("plan.failed");
    case "applied": {
      const parts = CHIPS.filter((op) => plan?.[op]).map((op) => t(done[op], plan?.[op] ?? 0));
      return t("plan.applied", parts.join(", ") || t("plan.appliedNothing"));
    }
    case "apply_failed":
      return t("plan.applyFailed");
    default:
      return "";
  }
}

/** One sentence: what the plan does, the most dangerous first, and where. */
export function sentence(plan: Plan): string {
  if (plan.stage === "plan_failed") return plan.error ?? t("plan.failedReason");
  if (plan.stage === "apply_failed") return plan.error ?? t("plan.applyFailedReason");
  if (plan.stage === "locked") return plan.locked_by ? t("plan.waitsFor", plan.locked_by) : t("plan.waitsOther");
  const verbs = { destroy: "plan.verb.destroy", replace: "plan.verb.replace", change: "plan.verb.change", add: "plan.verb.add" } as const;
  const kinds = new Map<PlanOp, Map<string, number>>();
  for (const group of plan.groups) {
    const byKind = kinds.get(group.op) ?? new Map<string, number>();
    byKind.set(group.kind, (byKind.get(group.kind) ?? 0) + group.count);
    kinds.set(group.op, byKind);
  }
  const pieces = DANGER.filter((op) => plan[op] > 0).map((op) => {
    const top = [...(kinds.get(op) ?? [])].sort((a, b) => b[1] - a[1]);
    if (plan.partial || top.length === 0) return `${t(verbs[op])} ${t("plan.resources", plan[op])}`;
    const shown = top.slice(0, 2);
    const rest = plan[op] - shown.reduce((sum, [, n]) => sum + n, 0);
    const named = shown.map(([kind, n]) => `${n} ${kindName(kind, n)}`);
    return `${t(verbs[op])} ${rest > 0 ? `${named.join(", ")} ${t("plan.more", rest)}` : both(named)}`;
  });
  if (pieces.length === 0) return t("plan.noChanges");
  const text = pieces.join("; ");
  const where = plan.projects > 1 ? t("plan.across", plan.projects, plan.unchanged) : plan.env ? t("plan.in", plan.env) : "";
  return `${text[0]?.toUpperCase()}${text.slice(1)}${where}.`;
}

/** The alerts the core found, in words; the first two are the serious ones. */
export function alerts(plan: Plan): { hot: boolean; text: string }[] {
  return plan.alerts.map((alert) => {
    switch (alert) {
      case "data":
        return { hot: true, text: t("alert.data", plan.production, both(plan.data.map((kind) => t("alert.dataItem", kindName(kind))))) };
      case "production":
        return { hot: true, text: t("alert.production") };
      case "drift":
        return { hot: false, text: t("alert.drift") };
      case "partial":
        return { hot: false, text: t("alert.partial") };
      case "no_detail":
        return { hot: false, text: t("alert.noDetail") };
    }
  });
}

/** "+3 ~1": the counts that are not zero, on a notice's line. */
export function planChips(plan: Plan): HTMLElement {
  const chips = el("span", "plan-chips plan-chips--mini");
  for (const op of CHIPS) if (plan[op] > 0) chips.append(el("span", `plan-chip plan-chip--${op} on`, `${SIGNS[op]}${plan[op]}`));
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
    for (const op of CHIPS) chips.append(el("span", `plan-chip plan-chip--${op}${plan[op] > 0 ? " on" : ""}`, `${SIGNS[op]}${plan[op]}`));
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
      item.append(el("span", `plan-op plan-op--${group.op}`, SIGNS[group.op]), el("span", "plan-what", group.count > 1 ? `${kindName(group.kind, group.count)} ×${group.count}` : group.address));
      list.append(item);
    }
    if (plan.projects > 1 && plan.unchanged > 0) list.append(el("li", "plan-project", t("plan.unchanged", plan.unchanged)));
    card.append(list);
  }
  const actions = el("div", "plan-actions");
  actions.append(button("btn-neutral", t("plan.openPr"), () => open(prUrl)));
  if (plan.comment_url) {
    const url = plan.comment_url;
    actions.append(button("btn-neutral", t("plan.full"), () => open(url)));
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
    if (plan.stage === "planning") runs.set(pull.url, { ...run, say: plan.projects > 0 ? t("run.planningDone", plan.projects) : t("run.planning"), going: true });
    else if (plan.stage === "applying") runs.set(pull.url, { ...run, say: t("run.applying"), going: true });
    else if (plan.stage === "locked") runs.set(pull.url, { ...run, say: plan.locked_by ? t("run.lockedBy", plan.locked_by) : t("run.locked"), going: false });
  }
  return [...runs.values()];
}
