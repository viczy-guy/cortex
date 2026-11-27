import { getVersion } from "@tauri-apps/api/app";

import { button, el } from "../shared/dom";
import { language, setLanguage, t } from "../shared/i18n";
import { api, releaseNotesUrl } from "../shared/ipc";
import { mascotSvg } from "../shared/mascots";
import { type Release, RELEASES } from "./news";
import { shown } from "./versions";

/** How long "Updating…" shows before the install starts, so it is seen at all. */
const UPDATING_MIN_MS = 1200;
/** How long "Update complete" shows before the same window grows into What's new (#180). */
const NEWS_AFTER_MS = 2000;

// The prefs first: the language of every text and the mascot (#154).
const prefs = await api.islandPrefs().catch(() => null);
setLanguage(prefs?.language ?? "en");
const current = await getVersion().catch(() => null);
const root = document.querySelector<HTMLElement>("#splash");
const params = new URLSearchParams(window.location.search);
const mode = params.get("mode");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
if (root && mode === "updating") updating(root);
else if (root && mode === "news") void news(root, shown(RELEASES, current, null));
else if (root) opening(root, mode === "updated", params.get("from"));

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

/** The app's version, blank if unavailable. */
const versionLine = (): HTMLElement => el("p", "version", current ? `v${current}` : "");

/**
 * "Welcome", or after an update (#127) straight to "Update complete". A
 * version with something to see then grows into What's new (#180); one with
 * fixes only offers them under a small "What changed".
 */
function opening(root: HTMLElement, updated: boolean, from: string | null): void {
  const releases = updated ? shown(RELEASES, current, from) : [];
  const grows = releases.some((r) => r.items.length > 0);
  const ready = el("div", "ready");
  ready.append(mascotBox(), el("p", undefined, updated ? t("splash.updated") : t("splash.ready")));
  if (releases.length > 0 && !grows) {
    const what = el("button", "what", t("news.what"));
    what.type = "button";
    what.addEventListener("click", (event) => {
      event.stopPropagation();
      stop();
      void news(root, releases);
    });
    ready.append(what);
  }
  if (updated) {
    root.classList.add("updated");
    root.append(ready, versionLine());
  } else {
    const dot = el("span", "opening-dot");
    dot.setAttribute("aria-hidden", "true");
    root.append(dot, el("p", "welcome", t("splash.welcome")), ready, versionLine());
  }

  let finished = false;
  if (grows) {
    // Not closed by a click or a timer: What's new waits for its own buttons.
    window.setTimeout(() => void news(root, releases), NEWS_AFTER_MS);
    return;
  }
  // The CSS delays (splash.css) and docs/SPEC.md follow this timeline; Rust
  // closes the window at 10 s if the page never does, so stay below that.
  const timer = window.setTimeout(finish, reducedMotion ? 3000 : updated ? 4000 : 6300);
  // Only a click: the window never takes the keyboard from the user's app.
  window.addEventListener("click", finish);

  function stop(): void {
    finished = true;
    window.clearTimeout(timer);
    window.removeEventListener("click", finish);
  }

  function finish(): void {
    if (finished) return;
    stop();
    api.splashDone().catch(() => { /* Rust closes the splash if IPC fails. */ });
  }
}

/**
 * What's new (#180): the newest version open, the ones skipped before it
 * folded. It stays until "Let's get to work" or the X, and never takes the
 * keyboard; the island appears meanwhile.
 */
async function news(root: HTMLElement, releases: Release[]): Promise<void> {
  // Rust makes the window bigger and keeps it; failing that, its 10 s fallback closes it.
  await api.splashNews().catch(() => { /* Drawn anyway. */ });
  const done = (): void => {
    api.splashDone().catch(() => { /* Rust closes the splash if IPC fails. */ });
  };
  const [latest, ...older] = releases;
  const head = el("header", "news-head");
  const title = el("div", "news-title");
  title.append(el("b", undefined, t("news.title", latest?.version ?? current ?? "")));
  if (latest) title.append(el("small", undefined, day(latest.date)));
  const close = el("button", "news-x", "\u00D7");
  close.type = "button";
  close.title = t("news.close");
  close.setAttribute("aria-label", t("news.close"));
  close.addEventListener("click", done);
  head.append(mascotBox(), title, close);

  const body = el("div", "news-body");
  if (latest) body.append(...release(latest, latest.items.length === 0));
  for (const r of older) {
    const section = el("details", "news-version");
    const summary = el("summary", undefined, `v${r.version} `);
    summary.append(el("small", undefined, `· ${day(r.date)}`));
    const items = el("div", "news-items");
    items.append(...release(r, true));
    section.append(summary, items);
    body.append(section);
  }

  const foot = el("footer", "news-foot");
  if (current) {
    const notes = releaseNotesUrl(current);
    foot.append(button("btn-neutral", t("news.github"), () => {
      api.openUrl(notes).catch(() => { /* Nothing to undo. */ });
    }));
  }
  foot.append(button("btn-allow", t("news.go"), done));
  root.className = "news";
  root.replaceChildren(head, body, foot);
}

/** A version's cards, then its fixes, folded unless `fixesOpen`. */
function release(r: Release, fixesOpen: boolean): HTMLElement[] {
  const lang = language();
  const nodes = r.items.map((item) => {
    const card = el("article", "news-item");
    if (item.image) {
      const image = el("img");
      image.src = item.image;
      image.alt = "";
      card.append(image);
    }
    const text = el("div", "news-text");
    text.append(el("b", undefined, item.title[lang]), el("span", undefined, item.text[lang]));
    card.append(text);
    return card;
  });
  if (r.fixes.length > 0) {
    const fixes = el("details", "news-fixes");
    fixes.open = fixesOpen;
    const summary = el("summary", undefined, `${t("news.fixes")} `);
    summary.append(el("small", undefined, `(${r.fixes.length})`));
    const list = el("ul");
    for (const fix of r.fixes) list.append(el("li", undefined, fix[lang]));
    fixes.append(summary, list);
    nodes.push(fixes);
  }
  return nodes;
}

/** "26 de novembro de 2026", "November 26, 2026". */
const day = (date: string): string =>
  new Intl.DateTimeFormat(language() === "pt" ? "pt-BR" : "en-US", { dateStyle: "long", timeZone: "UTC" }).format(new Date(`${date}T00:00:00Z`));

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
