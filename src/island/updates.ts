import { button, el } from "../shared/dom";
import type { UpdateStatus } from "../shared/ipc";

/** The first-run question is up until the user answers it, once. */
export function askingUpdates(status: UpdateStatus | null): boolean {
  return status !== null && status.enabled === null && status.state !== "unsupported";
}

export function updatesQuestion(answer: (enabled: boolean) => void, error: string | null): HTMLElement {
  const card = el("div", "card");
  const head = el("div", "card-head");
  head.append(el("span", "title", "Check for updates automatically?"));
  card.append(
    head,
    el(
      "div",
      "note",
      "Cortex would ask GitHub for a new release at launch and every 6 hours, and send nothing else. An update is installed only if it carries Cortex's signature, and only when you click to restart. You can change this in Settings.",
    ),
  );
  if (error) card.append(el("div", "note note-error", error));
  const footer = el("div", "actions");
  footer.append(button("btn-neutral", "No", () => answer(false)), button("btn-allow", "Yes", () => answer(true)));
  card.append(footer);
  return card;
}

/** A ready update: a click shows the update screen, which installs and restarts. */
export function updateChip(status: UpdateStatus | null, install: () => void): HTMLElement[] {
  return status?.state === "ready" ? [button("btn-chip", "Update and restart", install)] : [];
}
