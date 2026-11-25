import { button, el } from "../shared/dom";
import { fromRust, t } from "../shared/i18n";
import type { UpdateStatus } from "../shared/ipc";

/** The first-run question is up until the user answers it, once. */
export function askingUpdates(status: UpdateStatus | null): boolean {
  return status !== null && status.enabled === null && status.state !== "unsupported";
}

export function updatesQuestion(answer: (enabled: boolean) => void, error: string | null): HTMLElement {
  const card = el("div", "card");
  const head = el("div", "card-head");
  head.append(el("span", "title", t("updates.question")));
  card.append(head, el("div", "note", t("updates.questionNote")));
  if (error) card.append(el("div", "note note-error", fromRust(error)));
  const footer = el("div", "actions");
  footer.append(button("btn-neutral", t("common.no"), () => answer(false)), button("btn-allow", t("common.yes"), () => answer(true)));
  card.append(footer);
  return card;
}

/** A ready update: a click shows the update screen, which installs and restarts. */
export function updateChip(status: UpdateStatus | null, install: () => void): HTMLElement[] {
  return status?.state === "ready" ? [button("btn-chip", t("updates.restart"), install)] : [];
}
