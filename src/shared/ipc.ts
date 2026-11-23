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

/** How the island looks; sent to both windows when Settings changes it. */
export interface IslandPrefs {
  mascot: Mascot;
}
export const ISLAND_PREFS_EVENT = "cortex://island-prefs";

export interface UpdateStatus {
  /** `null` until the user answers the first-run question. */
  enabled: boolean | null;
  state: "unsupported" | "off" | "idle" | "checking" | "downloading" | "ready" | "error";
  /** The new version, while it downloads, once it is ready, or when installing it failed. */
  version: string | null;
  message: string | null;
}

export const UPDATES_EVENT = "cortex://updates";

export const api = {
  updatesStatus: () => invoke<UpdateStatus>("updates_status"),
  updatesSetEnabled: (enabled: boolean) => invoke<void>("updates_set_enabled", { enabled }),
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
  islandPrefs: () => invoke<IslandPrefs>("island_prefs"),
  islandSetMascot: (mascot: Mascot) => invoke<void>("island_set_mascot", { mascot }),
  splashDone: () => invoke<void>("splash_done"),
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
export interface PullRequest {
  repo: string;
  number: number;
  title: string;
  url: string;
  draft: boolean;
  author: string | null;
  checks: Checks;
  review: Review;
}
export interface GithubSummary {
  login: string;
  mine: PullRequest[];
  mine_total: number;
  review_requests: PullRequest[];
  review_requests_total: number;
  fetched_at_ms: number;
}
export interface AccountStatus {
  login: string;
  active: boolean;
  shown: boolean;
  state: "ok" | "error" | "signed_out" | "hidden";
  message: string | null;
  summary: GithubSummary | null;
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
