/**
 * Typed contract with the Rust core. These mirror the `Serialize` shapes in
 * `cortex-core` — if Rust changes a field, `npm run check` fails here instead
 * of the UI silently breaking.
 */

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

export interface PendingApproval {
  request_id: string;
  session: SessionKey;
  tool_name: string;
  detail: string;
  created_ms: number;
}

export interface Snapshot {
  sessions: SessionView[];
  approvals: PendingApproval[];
}

/** Decisions the UI can send for a pending approval. */
export type UiDecision = "allow" | "deny" | "always" | "ask";

/** Name of the state event emitted by the Rust side. */
export const STATE_EVENT = "cortex://state";
