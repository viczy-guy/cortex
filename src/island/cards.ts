/** Cards of the expanded panel: sessions, approvals and questions. */

import {
  type AlwaysRule,
  type Answers,
  api,
  type PendingApproval,
  type Question,
  type SessionView,
} from "../shared/ipc";
import { button, el } from "./dom";

export function stateLabel(state: SessionView["state"]): string {
  switch (state) {
    case "idle": return "idle";
    case "thinking": return "thinking…";
    case "working": return "working";
    case "waiting_approval": return "needs you";
    case "waiting_input": return "needs you";
    case "finished": return "done";
    case "error": return "error";
  }
}

export function sessionCard(session: SessionView): HTMLElement {
  const card = el("div", "card");

  const head = el("div", "card-head");
  head.append(el("span", `state-dot state-${session.state}`));
  head.append(el("span", "title", session.key.agent));
  head.append(el("span", "meta", stateLabel(session.state)));
  card.append(head);

  if (session.ticker.length > 0) {
    const steps = el("div", "steps");
    for (const step of session.ticker.slice(-4)) {
      steps.append(el("div", "step", step.label));
    }
    card.append(steps);
  }

  if (session.changes.length > 0) {
    const changes = el("div", "changes");
    for (const change of session.changes.slice(-6)) {
      const row = el("div", "change");
      row.append(el("span", "add", `+${change.added}`));
      row.append(el("span", "del", `−${change.removed}`));
      row.append(el("span", "path", change.path));
      changes.append(row);
    }
    card.append(changes);
  }

  return card;
}

export function approvalCard(approval: PendingApproval): HTMLElement {
  const card = el("div", "card approval");

  const head = el("div", "card-head");
  head.append(el("span", "state-dot state-waiting_approval"));
  head.append(el("span", "title", `${approval.tool_name} needs approval`));
  head.append(el("span", "meta", approval.session.agent));
  card.append(head);

  if (approval.detail) {
    card.append(el("div", "detail", approval.detail));
  }

  const id = approval.request_id;
  const actions = el("div", "actions");
  actions.append(button("btn-deny", "Deny", () => void api.respond(id, "deny")));
  actions.append(button("btn-allow", "Allow", () => void api.respond(id, "allow")));
  if (approval.always_rule) {
    const always = button("btn-neutral", alwaysLabel(approval.always_rule), () =>
      void api.respond(id, "always"),
    );
    always.title = alwaysScope(approval.always_rule);
    actions.append(always);
  }
  actions.append(terminalLink(id));
  card.append(actions);

  return card;
}

/** Exactly what an Always rule allows from now on, in words. */
export function alwaysScope(rule: AlwaysRule): string {
  if (rule.tool_name === "Bash") return `Every "${rule.pattern}" command, without asking`;
  if (rule.pattern) return `${rule.tool_name} on exactly ${rule.pattern}, without asking`;
  return `Every use of ${rule.tool_name}, without asking`;
}

/** Bash rules cover a whole program, so the button names it. */
function alwaysLabel(rule: AlwaysRule): string {
  if (rule.tool_name !== "Bash") return "Always";
  const program = rule.pattern.length > 18 ? `${rule.pattern.slice(0, 17)}…` : rule.pattern;
  return `Always allow ${program}`;
}

// ---------------------------------------------------------------------------
// Questions. Selections are view state: they live here, keyed by request id,
// so they survive re-renders and vanish with the card.

type Selection = Map<string, Set<string>>;
const selections = new Map<string, Selection>();

/** Forget selections of cards that no longer exist. */
export function pruneSelections(liveRequestIds: Set<string>): void {
  for (const id of selections.keys()) {
    if (!liveRequestIds.has(id)) selections.delete(id);
  }
}

export function questionCard(approval: PendingApproval, rerender: () => void): HTMLElement {
  const id = approval.request_id;
  const selection = selections.get(id) ?? new Map<string, Set<string>>();
  selections.set(id, selection);

  const card = el("div", "card approval");

  const head = el("div", "card-head");
  head.append(el("span", "state-dot state-waiting_approval"));
  head.append(el("span", "title", "Question"));
  head.append(el("span", "meta", approval.session.agent));
  card.append(head);

  const submit = () => void api.answer(id, toAnswers(approval.questions, selection));
  // One single-choice question: a click is the whole answer.
  const instant =
    approval.questions.length === 1 && !approval.questions[0]!.multi_select;

  for (const question of approval.questions) {
    const chosen = selection.get(question.question) ?? new Set<string>();
    selection.set(question.question, chosen);

    const block = el("div", "question");
    if (question.header) block.append(el("span", "question-header", question.header));
    block.append(el("div", "question-text", question.question));

    const options = el("div", "options");
    for (const option of question.options) {
      const selected = chosen.has(option.label);
      const node = button(selected ? "btn-option selected" : "btn-option", option.label, () => {
        if (question.multi_select) {
          if (selected) chosen.delete(option.label);
          else chosen.add(option.label);
        } else {
          chosen.clear();
          chosen.add(option.label);
        }
        if (instant) submit();
        else rerender();
      });
      if (option.description) node.title = option.description;
      options.append(node);
    }
    block.append(options);
    card.append(block);
  }

  const actions = el("div", "actions");
  if (!instant) {
    const complete = approval.questions.every(
      (q) => (selection.get(q.question)?.size ?? 0) > 0,
    );
    const send = button("btn-allow", "Send answer", submit);
    send.disabled = !complete;
    actions.append(send);
  }
  actions.append(terminalLink(id));
  card.append(actions);

  return card;
}

function toAnswers(questions: Question[], selection: Selection): Answers {
  const answers: Answers = {};
  for (const q of questions) {
    // Keep the option order of the question, not the click order.
    const chosen = selection.get(q.question) ?? new Set<string>();
    answers[q.question] = q.options
      .map((o) => o.label)
      .filter((label) => chosen.has(label))
      .join(", ");
  }
  return answers;
}

function terminalLink(requestId: string): HTMLElement {
  const link = el("span", "terminal-link", "answer in terminal");
  link.addEventListener("click", (event) => {
    event.stopPropagation();
    void api.respond(requestId, "ask");
  });
  return link;
}
