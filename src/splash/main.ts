import { getVersion } from "@tauri-apps/api/app";

import { el } from "../shared/dom";
import { api } from "../shared/ipc";
import { mascotSvg } from "../shared/mascots";

const root = document.querySelector<HTMLElement>("#splash");
if (root) {
  const dot = el("span", "opening-dot");
  dot.setAttribute("aria-hidden", "true");
  const welcome = el("p", "welcome", "Welcome");
  const ready = el("div", "ready");
  const mascot = el("div", "mascot");
  mascot.setAttribute("aria-hidden", "true");
  mascot.append(el("span", "mascot-dot"));
  ready.append(mascot, el("p", undefined, "Let's get to work?"));
  const version = el("p", "version");
  root.append(dot, welcome, ready, version);

  let finished = false;
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const timer = window.setTimeout(finish, reducedMotion ? 1200 : 2300);
  window.addEventListener("click", finish);
  window.addEventListener("keydown", finish);

  function finish(): void {
    if (finished) return;
    finished = true;
    window.clearTimeout(timer);
    window.removeEventListener("click", finish);
    window.removeEventListener("keydown", finish);
    api.splashDone().catch(() => { /* Rust closes the splash if IPC fails. */ });
  }

  api.islandPrefs().then((prefs) => {
    const svg = mascotSvg(prefs.mascot);
    if (svg) mascot.replaceChildren(svg);
  }).catch(() => { /* Keep the fallback dot. */ });

  getVersion().then((value) => {
    version.textContent = `v${value}`;
  }).catch(() => { /* Leave the version blank if unavailable. */ });
}
