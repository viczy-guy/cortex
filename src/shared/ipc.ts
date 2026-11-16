/**
 * Typed contract with the Rust core. The interfaces mirror the `Serialize`
 * shapes in `cortex-core` / `cortex-app`, and `api` is the only place the UI
 * calls into Rust — if a command or field changes, `npm run check` fails here
 * instead of the UI silently breaking.
 */

import { invoke } from "@tauri-apps/api/core";

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

/** Name of the state event emitted by the Rust side. */
export const STATE_EVENT = "cortex://state";

export const api = {
  rulesList: () => invoke<AlwaysRule[]>("rules_list"),
  ruleRemove: (rule: AlwaysRule) => invoke<void>("rule_remove", { rule }),
  rulesClear: () => invoke<void>("rules_clear"),
  islandFit: (width: number, height: number) => invoke<void>("island_fit", { width, height }),
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
