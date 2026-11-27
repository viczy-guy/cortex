/**
 * Typed contract with the Rust core. The interfaces mirror the `Serialize`
 * shapes in `cortex-core` / `cortex-app`, and `api` is the only place the UI
 * calls into Rust — if a command or field changes, `npm run check` fails here
 * instead of the UI silently breaking.
 */

import { invoke } from "@tauri-apps/api/core";

import type { Mascot } from "./mascots";

export type SessionState =
  | "idle"
  | "thinking"
  | "working"
  | "waiting_approval"
  | "waiting_input"
  | "finished"
  | "error";

export interface SessionKey {
  agent: string;
  session_id: string;
}

export interface Step {
  at_ms: number;
  label: string;
}

export interface FileChange {
  path: string;
  added: number;
  removed: number;
}

export interface SessionView {
  key: SessionKey;
  state: SessionState;
  prompt: string | null;
  current: string | null;
  ticker: Step[];
  changes: FileChange[];
  cwd: string | null;
  started_at_ms: number;
  last_event_ms: number;
}

export interface QuestionOption {
  label: string;
  description: string;
}

export interface Question {
  question: string;
  header: string;
  options: QuestionOption[];
  multi_select: boolean;
}

/** A card waiting for the user: an approval, or a question when `questions` is not empty. */
export interface PendingApproval {
  request_id: string;
  session: SessionKey;
  tool_name: string;
  detail: string;
  questions: Question[];
  /** The exact change of an Edit, or the whole content of a Write. */
  diff: DiffLine[];
  /** Diff lines left out for size. */
  diff_hidden: number;
  /** What "Always" would remember; `null` when no safe rule exists. */
  always_rule: AlwaysRule | null;
  created_ms: number;
}

/** An "Always allow" rule: for Bash, `pattern` is the program name. */
export interface AlwaysRule {
  tool_name: string;
  pattern: string;
}

export interface Snapshot {
  sessions: SessionView[];
  approvals: PendingApproval[];
}

/** Decisions the UI can send for a pending approval. */
export type UiDecision = "allow" | "deny" | "always" | "ask";

/** Question text → chosen label (multi-select: labels joined by ", "). */
export type Answers = Record<string, string>;

export interface DiffLine {
  kind: "same" | "added" | "removed";
  text: string;
}

export interface HooksStatus {
  installed: boolean;
  outdated: boolean;
  settings_path: string;
}

export interface HooksPreview {
  diff: DiffLine[];
  token: string;
  settings_path: string;
  changes: boolean;
}

export interface HooksInstalled {
  backup_path: string | null;
}

export const RULES_EVENT = "cortex://rules";
export const HOOKS_EVENT = "cortex://hooks";

/** Pointer entered (`true`) or left (`false`) the island — macOS only. */
export const HOVER_EVENT = "cortex://hover";

/** A click in another app while a card waits — macOS only. */
export const OUTSIDE_CLICK_EVENT = "cortex://outside-click";

/** Name of the state event emitted by the Rust side. */
export const STATE_EVENT = "cortex://state";

export type IslandPosition = "top" | "left" | "right" | "bottom-left" | "bottom-right";

export interface IslandCutout {
  width: number;
  height: number;
}

export const ISLAND_CUTOUT_EVENT = "cortex://island-cutout";

export interface IslandScreen {
  name: string;
  primary: boolean;
  width: number;
  height: number;
}

/** How the island looks and where it sits; sent when Settings changes it. */
export interface IslandPrefs {
  mascot: Mascot;
  position: IslandPosition;
  screen: string | null;
  sound: boolean;
  /** The lively mascot (#145). */
  lively: boolean;
  /** The language shown (#154): the one picked, else the system's. */
  language: "en" | "pt";
  /** The one picked in Settings; `null` follows the system. */
  language_choice: "en" | "pt" | null;
}
export const ISLAND_PREFS_EVENT = "cortex://island-prefs";

/** Where the lively mascot looks (#145): sent when it changes, and once a minute while the mouse moves. */
export interface Gaze {
  /** -1 (left) to 1 (right). */
  x: number;
  /** -1 (up) to 1 (down). */
  y: number;
  near: boolean;
  /** `false` while the screen is locked: nothing may repaint (#51). */
  awake: boolean;
}
export const GAZE_EVENT = "cortex://gaze";
export const ISLAND_SCREENS_EVENT = "cortex://island-screens";

export interface AutostartStatus {
  enabled: boolean;
  /** `false` for a dev build or an app still on its disk image. */
  available: boolean;
}

export interface UpdateStatus {
  /** `null` until the user answers the first-run question. */
  enabled: boolean | null;
  state: "unsupported" | "off" | "idle" | "checking" | "downloading" | "ready" | "error";
  /** The new version, while it downloads and once it is ready. */
  version: string | null;
  message: string | null;
}

export const UPDATES_EVENT = "cortex://updates";
export const OPENING_CLOSED_EVENT = "cortex://opening-closed";
/** Nobody is looking (`true`: locked, idle ten minutes, asleep) or somebody is back (#157). */
export const AWAY_EVENT = "cortex://away";

export const api = {
  updatesStatus: () => invoke<UpdateStatus>("updates_status"),
  updatesSetEnabled: (enabled: boolean) => invoke<void>("updates_set_enabled", { enabled }),
  /** Shows the update screen, which then installs. */
  updatesStart: () => invoke<void>("updates_start"),
  /** The update screen only: install and restart; rejects with why it failed. */
  updatesInstall: () => invoke<void>("updates_install"),
  githubStatus: () => invoke<GithubStatus>("github_status"),
  githubSetEnabled: (enabled: boolean) => invoke<void>("github_set_enabled", { enabled }),
  githubSetAccountShown: (login: string, shown: boolean) => invoke<void>("github_set_account_shown", { login, shown }),
  githubSignIn: () => invoke<void>("github_sign_in"),
  githubInstall: () => invoke<void>("github_install"),
  githubCancelTask: () => invoke<void>("github_cancel_task"),
  githubRefresh: () => invoke<void>("github_refresh"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  rulesList: () => invoke<AlwaysRule[]>("rules_list"),
  ruleRemove: (rule: AlwaysRule) => invoke<void>("rule_remove", { rule }),
  rulesClear: () => invoke<void>("rules_clear"),
  islandFit: (width: number, height: number) => invoke<void>("island_fit", { width, height }),
  /** Opened by a click, the island stays open until a click outside (#173). */
  islandPin: (pinned: boolean) => invoke<void>("island_pin", { pinned }),
  islandCutout: () => invoke<IslandCutout | null>("island_cutout"),
  islandPrefs: () => invoke<IslandPrefs>("island_prefs"),
  islandSetMascot: (mascot: Mascot) => invoke<void>("island_set_mascot", { mascot }),
  islandScreens: () => invoke<IslandScreen[]>("island_screens"),
  islandSetPosition: (position: IslandPosition) => invoke<void>("island_set_position", { position }),
  islandSetScreen: (screen: string | null) => invoke<void>("island_set_screen", { screen }),
  islandSetSound: (enabled: boolean) => invoke<void>("island_set_sound", { enabled }),
  islandSetLively: (enabled: boolean) => invoke<void>("island_set_lively", { enabled }),
  islandGazeOrigin: (x: number, y: number) => invoke<void>("island_gaze_origin", { x, y }),
  autostartStatus: () => invoke<AutostartStatus>("autostart_status"),
  autostartSet: (enabled: boolean) => invoke<AutostartStatus>("autostart_set", { enabled }),
  splashDone: () => invoke<void>("splash_done"),
  /** What's new (#180): the screen grows and stays until its buttons; the island appears. */
  splashNews: (height: number) => invoke<void>("splash_news", { height }),
  /** Settings → About → What's new (#180). */
  newsOpen: () => invoke<void>("news_open"),
  /** The launch's opening screen is still up: the island waits (#152). */
  openingShown: () => invoke<boolean>("opening_shown"),
  languageSet: (language: "en" | "pt" | null) => invoke<void>("language_set", { language }),
  awayNow: () => invoke<boolean>("away_now"),
  openSettings: () => invoke<void>("open_settings"),
  quit: () => invoke<void>("quit"),
  snapshot: () => invoke<Snapshot>("snapshot"),
  respond: (requestId: string, decision: UiDecision) =>
    invoke<void>("respond", { requestId, decision }),
  answer: (requestId: string, answers: Answers) =>
    invoke<void>("answer", { requestId, answers }),
  hooksStatus: () => invoke<HooksStatus>("hooks_status"),
  hooksPreview: () => invoke<HooksPreview>("hooks_preview"),
  hooksUninstallPreview: () => invoke<HooksPreview>("hooks_uninstall_preview"),
  hooksUninstall: (token: string) =>
    invoke<HooksInstalled>("hooks_uninstall", { token }),
  hooksInstall: (token: string) => invoke<HooksInstalled>("hooks_install", { token }),
};

export type Checks = "passing" | "failing" | "pending" | "none";
export type Review = "approved" | "changes_requested" | "required" | "none";
/** What a plan does to a resource, the most dangerous first (#161). */
export type PlanOp = "destroy" | "replace" | "change" | "add";
export interface PlanGroup {
  /** The project, when the plan has several with changes: "production · data/rds". */
  project: string | null;
  op: PlanOp;
  /** The resource type: "aws_instance". */
  kind: string;
  count: number;
  /** The first resource's address, shown when it is alone. */
  address: string;
}
/** A project of the run shown (#173): the plan's, or the apply's once it applies. */
export interface AtlantisRun {
  name: string;
  /** The folders after the region: "data/rds", else the name. */
  label: string;
  env: string | null;
  state: "running" | "failed" | "queued" | "done" | "skipped";
  /** Its plan's counts, once planned. */
  add: number;
  change: number;
  destroy: number;
  /** Its log on the Atlantis server, live while it runs. */
  url: string | null;
}
/** What runs now in a plan or an apply (#173). */
export interface AtlantisLive {
  /** Atlantis prepares (its pre-workflow hooks), before the first project. */
  preparing: boolean;
  project: string | null;
  /** Since when, as GitHub writes it (ISO 8601). */
  since: string;
  url: string | null;
}
/** A pull request's latest Atlantis run (#161), from `cortex_core::atlantis`. */
export interface Plan {
  stage: "planning" | "planned" | "plan_failed" | "locked" | "applying" | "applied" | "apply_failed";
  add: number;
  change: number;
  replace: number;
  destroy: number;
  /** Projects planned so far, and those of them without changes. */
  projects: number;
  unchanged: number;
  env: string | null;
  production: boolean;
  /** Resource types whose data a destroy or a replace loses. */
  data: string[];
  /** What deserves care, the most serious first. */
  alerts: ("data" | "production" | "drift" | "partial" | "no_detail")[];
  groups: PlanGroup[];
  /** The list shows part of the plan, or none of it. */
  partial: boolean;
  error: string | null;
  locked_by: number | null;
  comment_url: string | null;
  /** Every account its projects are in (the folder before the region), production first (#173). */
  envs: { name: string; production: boolean }[];
  /** While a plan or an apply runs: what runs now. */
  live: AtlantisLive | null;
  /** The projects of the run shown: running, failed, queued, done, skipped. */
  runs: AtlantisRun[];
  /** Projects left out of `runs` for size. */
  runs_hidden: number;
  /** Of the run shown, the projects done and those it runs, counted before `runs` is cut. */
  runs_done: number;
  runs_total: number;
}
export interface PullRequest {
  repo: string;
  number: number;
  title: string;
  url: string;
  draft: boolean;
  author: string | null;
  checks: Checks;
  review: Review;
  plan: Plan | null;
}
/** Something on GitHub worth a notice (#131), most urgent first. */
export interface GithubNotice {
  /** The same while the notice stands; a new failure or plan gets a new one. */
  key: string;
  kind: "review" | "approved" | "failed" | "plan_ready" | "plan_destroys" | "plan_failed" | "applied" | "apply_failed" | "action";
  /** Where it happened: "cortex #128 · checks". */
  detail: string;
  url: string;
  /** The pull request's number and short title (0 and "" for a failed action); "approved" names them in its phrase. */
  number: number;
  title: string;
  /** The Atlantis run it tells about; on a review request, its plan when that destroys nothing. */
  plan?: Plan;
}
/** A pull request in the island's Atlantis tab (#173). */
export interface Watched {
  pull: PullRequest;
  /** The user's own; else it asks for their review. */
  own: boolean;
  merged: boolean;
  /** 0 while its run goes, 1 locked, 2 moved in the last hour; `at`, when it moved (ISO 8601). */
  order: number;
  at: string;
}
export interface GithubSummary {
  login: string;
  mine: PullRequest[];
  mine_total: number;
  review_requests: PullRequest[];
  review_requests_total: number;
  fetched_at_ms: number;
  notices: GithubNotice[];
  /** Runs going first, then locked plans, then what moved in the last hour (#173). */
  atlantis: Watched[];
}
export interface AccountStatus {
  login: string;
  active: boolean;
  shown: boolean;
  state: "ok" | "error" | "signed_out" | "hidden";
  message: string | null;
  summary: GithubSummary | null;
  /** On the first answer after launch only: the notices that came while Cortex was closed (#157). */
  new_since_last_run?: string[];
}
export type GithubTask =
  | { kind: "idle" }
  | { kind: "installing" }
  | { kind: "signing_in"; code: string | null }
  | { kind: "failed"; message: string };
export interface GithubStatus {
  enabled: boolean;
  state: "off" | "checking" | "ok" | "no_gh" | "signed_out" | "error";
  message: string | null;
  accounts: AccountStatus[];
  installer: "brew" | "winget" | null;
  task: GithubTask;
}
export const GITHUB_DEVICE_URL = "https://github.com/login/device";
export const GITHUB_CLI_URL = "https://cli.github.com/";
/** Must match `release_notes_url` in cortex-core: `open_url` accepts only that page. */
export const releaseNotesUrl = (version: string): string => `https://github.com/viczy-guy/cortex/releases/tag/v${version}`;
export const GITHUB_EVENT = "cortex://github";
/** A status as drawn: `fetched_at_ms` changes on every check (15 s) but is never shown. */
export const githubShown = (status: GithubStatus | null): string =>
  JSON.stringify(status, (key, value: unknown) => (key === "fetched_at_ms" ? undefined : value));
