/**
 * "Connect Claude Code": review the exact change to the settings file, then
 * install. Nothing is written until the user clicks Install on a diff they
 * can read — the write itself is done (and re-checked) by Rust.
 */

import type { HooksPreview } from "../shared/ipc";
import { button, el } from "./dom";

export type Onboarding =
  | { phase: "closed" }
  | { phase: "review"; preview: HooksPreview }
  | { phase: "done"; backupPath: string | null }
  | { phase: "error"; message: string };

export interface OnboardingActions {
  install: (token: string) => void;
  close: () => void;
}

export function onboardingCard(
  state: Exclude<Onboarding, { phase: "closed" }>,
  actions: OnboardingActions,
): HTMLElement {
  const card = el("div", "card");
  const head = el("div", "card-head");
  head.append(el("span", "title", "Connect Claude Code"));
  card.append(head);

  const footer = el("div", "actions");

  switch (state.phase) {
    case "review": {
      const { preview } = state;
      if (!preview.changes) {
        card.append(el("div", "note", "Hooks are already installed. Nothing to change."));
        footer.append(button("btn-neutral", "Close", actions.close));
        break;
      }
      card.append(
        el(
          "div",
          "note",
          `Cortex will add its hooks to ${preview.settings_path}. A dated backup is saved first; nothing else in the file changes.`,
        ),
      );
      card.append(diffView(preview));
      footer.append(button("btn-neutral", "Cancel", actions.close));
      footer.append(button("btn-allow", "Install hooks", () => actions.install(preview.token)));
      break;
    }
    case "done":
      card.append(
        el("div", "note", "Hooks installed. New Claude Code sessions will show up here."),
      );
      if (state.backupPath) {
        card.append(el("div", "note note-dim", `Backup: ${state.backupPath}`));
      }
      footer.append(button("btn-neutral", "Close", actions.close));
      break;
    case "error":
      card.append(el("div", "note note-error", state.message));
      footer.append(button("btn-neutral", "Close", actions.close));
      break;
  }

  card.append(footer);
  return card;
}

/** The diff, opened at the first change. Unchanged lines far from a change are folded. */
function diffView(preview: HooksPreview): HTMLElement {
  const CONTEXT = 2;
  const view = el("div", "diff");
  const { diff } = preview;

  const nearChange = diff.map((_, i) =>
    diff
      .slice(Math.max(0, i - CONTEXT), i + CONTEXT + 1)
      .some((line) => line.kind !== "same"),
  );

  let folded = false;
  diff.forEach((line, i) => {
    if (!nearChange[i]) {
      if (!folded) view.append(el("div", "diff-line diff-fold", "⋯"));
      folded = true;
      return;
    }
    folded = false;
    const sign = line.kind === "added" ? "+" : line.kind === "removed" ? "−" : " ";
    view.append(el("div", `diff-line diff-${line.kind}`, `${sign} ${line.text}`));
  });
  return view;
}
