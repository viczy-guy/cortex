/**
 * "Connect Claude Code": review the exact change to the settings file, then
 * install. Nothing is written until the user clicks Install on a diff they
 * can read — the write itself is done (and re-checked) by Rust.
 */

import { diffView } from "../shared/diff-view";
import type { HooksPreview } from "../shared/ipc";
import { button, el } from "../shared/dom";

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
      card.append(diffView(preview.diff));
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
