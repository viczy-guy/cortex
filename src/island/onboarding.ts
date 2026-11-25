/**
 * "Connect Claude Code": review the exact change to the settings file, then
 * install. Nothing is written until the user clicks Install on a diff they
 * can read — the write itself is done (and re-checked) by Rust.
 */

import { diffView } from "../shared/diff-view";
import type { HooksPreview } from "../shared/ipc";
import { button, el } from "../shared/dom";
import { fromRust, t } from "../shared/i18n";

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
  head.append(el("span", "title", t("island.connect")));
  card.append(head);

  const footer = el("div", "actions");

  switch (state.phase) {
    case "review": {
      const { preview } = state;
      if (!preview.changes) {
        card.append(el("div", "note", t("hooks.already")));
        footer.append(button("btn-neutral", t("common.close"), actions.close));
        break;
      }
      card.append(
        el("div", "note", t("hooks.willAddTo", preview.settings_path)),
      );
      card.append(diffView(preview.diff));
      footer.append(button("btn-neutral", t("common.cancel"), actions.close));
      footer.append(button("btn-allow", t("hooks.install"), () => actions.install(preview.token)));
      break;
    }
    case "done":
      card.append(
        el("div", "note", t("hooks.installed")),
      );
      if (state.backupPath) {
        card.append(el("div", "note note-dim", t("hooks.backup", state.backupPath)));
      }
      footer.append(button("btn-neutral", t("common.close"), actions.close));
      break;
    case "error":
      card.append(el("div", "note note-error", fromRust(state.message)));
      footer.append(button("btn-neutral", t("common.close"), actions.close));
      break;
  }

  card.append(footer);
  return card;
}
