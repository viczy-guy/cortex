import { getVersion } from "@tauri-apps/api/app";

import { button, el } from "../shared/dom";
import { setLanguage, t } from "../shared/i18n";
import { api } from "../shared/ipc";
import { mascotSvg } from "../shared/mascots";

/** How long "Updating…" shows before the install starts, so it is seen at all. */
const UPDATING_MIN_MS = 1200;

// The prefs first: the language of every text and the mascot (#154).
const prefs = await api.islandPrefs().catch(() => null);
setLanguage(prefs?.language ?? "en");
const root = document.querySelector<HTMLElement>("#splash");
const mode = new URLSearchParams(window.location.search).get("mode");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
if (root && mode === "updating") updating(root);
else if (root) opening(root, mode === "updated");

/** The selected mascot, the dot as a fallback, nothing for "No mascot". */
function mascotBox(): HTMLElement {
  const mascot = el("div", "mascot");
  mascot.setAttribute("aria-hidden", "true");
  if (!prefs) {
    mascot.append(el("span", "mascot-dot"));
  } else {
    const svg = mascotSvg(prefs.mascot, "splash-mascot");
    if (svg) mascot.append(svg); // "No mascot": nothing jumps there.
  }
  return mascot;
}

function versionLine(): HTMLElement {
  const version = el("p", "version");
  getVersion().then((value) => {
    version.textContent = `v${value}`;
  }).catch(() => { /* Leave the version blank if unavailable. */ });
  return version;
}

/** "Welcome", or after an update (#127) straight to "Update complete". */
function opening(root: HTMLElement, updated: boolean): void {
  const ready = el("div", "ready");
  ready.append(mascotBox(), el("p", undefined, updated ? t("splash.updated") : t("splash.ready")));
  if (updated) {
    root.classList.add("updated");
    root.append(ready, versionLine());
  } else {
    const dot = el("span", "opening-dot");
    dot.setAttribute("aria-hidden", "true");
    root.append(dot, el("p", "welcome", t("splash.welcome")), ready, versionLine());
  }

  let finished = false;
  // The CSS delays (splash.css) and docs/SPEC.md follow this timeline; Rust
  // closes the window at 10 s if the page never does, so stay below that.
  const timer = window.setTimeout(finish, reducedMotion ? 3000 : updated ? 4000 : 6300);
  // Only a click: the window never takes the keyboard from the user's app.
  window.addEventListener("click", finish);

  function finish(): void {
    if (finished) return;
    finished = true;
    window.clearTimeout(timer);
    window.removeEventListener("click", finish);
    api.splashDone().catch(() => { /* Rust closes the splash if IPC fails. */ });
  }
}

/**
 * "Updating…" while Rust installs; on success the app restarts and this page
 * goes with it. A failure never keeps the user from working: they can try
 * again or go on with the version they have.
 */
function updating(root: HTMLElement): void {
  root.classList.add("update");
  const view = el("div", "stack");
  root.append(view);
  const versions = el("p", "dim");
  const mascot = mascotBox();
  Promise.all([getVersion(), api.updatesStatus()]).then(([from, status]) => {
    if (status.version) versions.textContent = `${from} → ${status.version}`;
  }).catch(() => { /* The versions are a nicety. */ });

  void install();

  async function install(): Promise<void> {
    root.classList.remove("failed");
    const track = el("span", "track");
    track.append(el("i"));
    view.replaceChildren(mascot, el("p", "title", t("splash.updating")), track, versions);
    await new Promise((resolve) => window.setTimeout(resolve, UPDATING_MIN_MS));
    try {
      await api.updatesInstall();
    } catch {
      failed();
    }
  }

  function failed(): void {
    root.classList.add("failed");
    const actions = el("div", "actions");
    actions.append(
      button("btn-neutral", t("splash.continue"), () => {
        api.splashDone().catch(() => { /* Rust closes the splash if IPC fails. */ });
      }),
      button("btn-allow", t("splash.retry"), () => void install()),
    );
    view.replaceChildren(
      mascot,
      el("p", "title", t("splash.oops")),
      el("p", "dim", t("splash.whenever")),
      actions,
    );
  }
}
