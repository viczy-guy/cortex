/**
 * The lively mascot (#145): eyes on the mouse, blinks, a look around, a nap,
 * and a reaction to clicks and to the island's color. One SVG lives across
 * renders, so its eyes and moves survive the redraws. Nothing runs while the
 * mascot is not lively or naps, and nothing is touched while the screen is
 * locked: a repaint of a locked island keeps WebKit busy until unlock (#51).
 */

import { api, type Gaze } from "../shared/ipc";
import { type Mascot, mascotReach, mascotSvg } from "../shared/mascots";

/** No agent busy, no notice standing and no mouse for this long: a nap. */
const NAP_MS = 10 * 60_000;
const NAP_CHECK_MS = 30_000;
const BLINK_MS = 120;
const CLICKS_FOR_DIZZY = 3;
const DIZZY_WINDOW_MS = 1500;
const DIZZY_MS = 2000;const MOVES = ["boop", "shake", "wobble"] as const;

type Mood = "happy" | "wide" | "dizzy";
type Move = (typeof MOVES)[number];

let node: SVGSVGElement | null = null;
let drawn: Mascot | null = null;
let reach = mascotReach("eyes");
let lively = true;
/** The screen is unlocked. */
let awake = true;
let napping = false;
let mood: Mood | null = null;
let aim: [number, number] = [0, 0];
let near = false;
let lastMove = Date.now();
let lastBusy = Date.now();
/** Agents at work or a notice standing, as of the last render. */
let busyNow = false;
let clicks: number[] = [];
let origin = "";
let moodTimer: number | undefined;
let blinkTimer: number | undefined;
let glanceTimer: number | undefined;
let napTimer: number | undefined;

const rand = (low: number, high: number): number => low + Math.random() * (high - low);
const live = (): boolean => lively && awake && node !== null;
const calm = (): boolean => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** The mascot to draw: the same node until the user picks another. */
export function livelyMascot(mascot: Mascot): SVGSVGElement | null {
  if (mascot !== drawn) {
    drawn = mascot;
    reach = mascotReach(mascot);
    node = mascotSvg(mascot);
    if (node) dress(node);
    paint();
    schedule();
  }
  // Once it is in place: Rust measures the eyes' direction from here.
  if (node) queueMicrotask(report);
  return node;
}

function dress(svg: SVGSVGElement): void {
  if (svg.querySelector(".m-closed")) svg.classList.add("has-closed");
  if (svg.querySelector(".m-happy")) svg.classList.add("has-happy");
  const z = document.createElementNS("http://www.w3.org/2000/svg", "text");
  z.setAttribute("class", "m-z");
  z.setAttribute("x", "33");
  z.setAttribute("y", "5");
  z.textContent = "z";
  svg.append(z);
  // A move is played once: left on, a redraw that moves the node would play it again.
  svg.addEventListener("animationend", (event) => {
    const target = event.target;
    if (target === svg) svg.classList.remove("bounce");
    else if (target instanceof Element && target.classList.contains("m-body")) target.classList.remove(...MOVES);
  });
}

// The window grows and shrinks after a render, and folds back without one:
// the mascot's place is read again whenever its size changes.
window.addEventListener("resize", () => queueMicrotask(report));

/** The mascot's center in the island, sent when it moves. */
function report(): void {
  if (!node?.isConnected || !lively) return;
  const box = node.getBoundingClientRect();
  if (box.width === 0) return;
  const x = Math.round(box.left + box.width / 2);
  const y = Math.round(box.top + box.height / 2);
  const key = `${x},${y}`;
  if (key === origin) return;
  origin = key;
  void api.islandGazeOrigin(x, y).catch(() => { origin = ""; });
}

export function setLively(on: boolean): void {
  if (on === lively) return;
  lively = on;
  if (!on) {
    napping = false;
    mood = null;
    aim = [0, 0];
    near = false;
  } else origin = "";
  paint();
  schedule();
}

/** From Rust: a new direction, a mouse still moving, or the screen locking or unlocking. */
export function onGaze(gaze: Gaze): void {
  if (!lively) return;
  if (!gaze.awake) {
    if (awake) {
      awake = false;
      window.clearTimeout(moodTimer);
      schedule();
    }
    return;
  }
  const unlocked = !awake;
  awake = true;
  lastMove = Date.now();
  aim = [gaze.x, gaze.y];
  near = gaze.near;
  if (unlocked) settle();
  node?.classList.toggle("near", near);
  if (napping && (near || busyNow)) wakeUp();
  else if (!napping && mood !== "dizzy") look(...aim);
}

/** Back from a locked screen: what was cut short ends now. */
function settle(): void {
  mood = null;
  node?.classList.remove("blink");
  paint();
  schedule();
}

/** Agents at work or a notice standing: no nap, and a napping mascot wakes. */
export function setBusy(on: boolean): void {
  busyNow = on;
  if (!on) return;
  lastBusy = Date.now();
  if (napping && live()) wakeUp();
}

/** A click on the island: a squash and a smile; three quick ones, dizzy. */
export function poke(): void {
  if (!live()) return;
  if (napping) {
    wakeUp();
    feel("wide", 700);
    return;
  }
  if (mood === "dizzy") return;
  const now = Date.now();
  clicks = clicks.filter((at) => now - at < DIZZY_WINDOW_MS);
  clicks.push(now);
  if (clicks.length >= CLICKS_FOR_DIZZY && !calm()) {
    clicks = [];
    move("wobble");
    feel("dizzy", DIZZY_MS);
    return;
  }
  move("boop");
  feel("happy", 1000);
}

/**
 * The island's color rose: the jump it always had and, when the mascot is
 * lively, eyes that say why: a smile for green, wide for the rest, a shake
 * for red. Nothing while the screen is locked.
 */
export function react(tone: "green" | "blue" | "amber" | "red"): void {
  if (!node || (lively && !awake)) return;
  setBusy(true);
  if (lively && tone === "red") move("shake");
  else jump();
  if (live()) feel(tone === "green" ? "happy" : "wide", 900);
}

function jump(): void {
  // Reduce motion: no jump, and no class left on that would play one later.
  if (!node || calm()) return;
  node.classList.remove("bounce");
  void node.getBoundingClientRect();
  node.classList.add("bounce");
}

function wakeUp(): void {
  napping = false;
  lastMove = Date.now();
  paint();
  blink();
  window.setTimeout(blink, 300);
  jump();
  schedule();
}

function feel(next: Mood, ms: number): void {
  window.clearTimeout(moodTimer);
  mood = next;
  paint();
  moodTimer = window.setTimeout(() => {
    if (!live()) return;
    mood = null;
    paint();
  }, ms);
}

function move(kind: Move): void {
  const body = node?.querySelector<SVGGElement>(".m-body");
  if (!body || calm()) return;
  body.classList.remove(...MOVES);
  void body.getBoundingClientRect();
  body.classList.add(kind);
}

/** Eyes (or the whole head, for a mascot without separate eyes) toward -1..1 each way. */
function look(lx: number, ly: number): void {
  if (!node) return;
  const target = node.querySelector<SVGGElement>(".m-look") ?? node.querySelector<SVGGElement>(".m-head");
  if (!target) return;
  let x = lx * reach.reach[0];
  let y = ly * reach.reach[1];
  if (reach.pixel) {
    x = Math.round(lx * 1.4);
    y = Math.round(ly * 1.2);
  }
  shift(target, x, y);
  // The eyebrows go with the eyes, like a face: across, and up when they look up. Never
  // down, onto the glasses, nor up while a mood lifts them already (into the ninja's mask).
  const brows = node.querySelector<SVGGElement>(".m-follow");
  if (brows && target.classList.contains("m-look")) shift(brows, x, mood ? 0 : Math.min(y, 0));
}

function shift(target: SVGGElement, x: number, y: number): void {
  const transform = x === 0 && y === 0 ? "" : `translate(${x.toFixed(2)}px, ${y.toFixed(2)}px)`;
  if (target.style.transform !== transform) target.style.transform = transform;
}

function blink(): void {
  if (!live() || napping || mood) return;
  node?.classList.add("blink");
  window.setTimeout(() => { if (awake) node?.classList.remove("blink"); }, BLINK_MS);
}

/** The classes for the mood, the nap and the eyes, on the node as it is now. */
function paint(): void {
  if (!node) return;
  const on = lively;
  for (const name of ["happy", "wide", "dizzy"] as const) node.classList.toggle(name, on && mood === name);
  node.classList.toggle("asleep", on && napping);
  node.classList.toggle("near", on && near);
  if (!on) node.classList.remove("blink");
  if (!on) look(0, 0);
  else if (napping) look(0, 0.5);
  else if (mood !== "dizzy") look(...aim);
}

/** Timers run only while the mascot is lively, the screen unlocked and the mascot awake. */
function schedule(): void {
  window.clearTimeout(blinkTimer);
  window.clearTimeout(glanceTimer);
  window.clearTimeout(napTimer);
  if (!live() || napping) return;
  blinks();
  glances();
  naps();
}

function blinks(): void {
  blinkTimer = window.setTimeout(() => {
    blink();
    if (Math.random() < 0.22) window.setTimeout(blink, 280);
    blinks();
  }, rand(2500, 6000));
}

/** With the mouse at rest, a look to one side now and then. */
function glances(): void {
  glanceTimer = window.setTimeout(() => {
    if (live() && !napping && !mood && Date.now() - lastMove > 4000) {
      look(rand(-1, 1), rand(-0.6, 0.8));
      window.setTimeout(() => { if (live() && !mood && !napping) look(...aim); }, 1300);
    }
    glances();
  }, rand(5000, 10_000));
}

function naps(): void {
  napTimer = window.setTimeout(() => {
    const now = Date.now();
    if (busyNow) lastBusy = now;
    if (now - lastMove >= NAP_MS && now - lastBusy >= NAP_MS) {
      napping = true;
      mood = null;
      paint();
      schedule();
      return;
    }
    naps();
  }, NAP_CHECK_MS);
}
