import type { DiffLine } from "./ipc";
import { el } from "./dom";

/** The diff, opened at the first change. Unchanged lines far from a change are folded. */
export function diffView(diff: DiffLine[]): HTMLElement {
  const CONTEXT = 2;
  const view = el("div", "diff");

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
