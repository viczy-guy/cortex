/**
 * The lively mascot (#145): eyes on the mouse, blinks, a look around, a nap,
 * and a reaction to clicks and to the island's color. One SVG lives across
 * renders, so its eyes and moves survive the redraws. Nothing runs while the
 * mascot is not lively, while it naps, or while the screen is locked: a
 * repaint of a locked island keeps WebKit busy until unlock (#51).
 */

import type { Gaze } from "../shared/ipc";
import { type Mascot, mascotReach, mascotSvg } from "../shared/mascots";

/** `cortex_core::gaze` steps on each side. */
const STEPS_X = 4;
const STEPS_Y = 3;
/** No agent busy, no notice standing and no mouse for this long: a nap. */
const NAP_MS = 10 * 60_000;
const NAP_CHECK_MS = 30_000;
const BLINK_MS = 120;
const CLICKS_FOR_DIZZY = 3;
const DIZZY_WINDOW_MS = 1500;
const DIZZY_MS = 2000;

type Mood = "happy" | "wide" | "dizzy";
type Move = "hop" | "boop" | "shake" | "wobble";

let node: SVGSVGElement | null = null;
let drawn: Mascot | null = null;
let reach = mascotReach("eyes");
let lively = true;
/** The screen is unlocked. */
let awake = true;
let napping = false;
let mood: Mood | null = null;
let aim: [number, number] = [0, 0];
let lastMove = Date.now();
let lastBusy = Date.now();
/** Agents at work or a notice standing, as of the last render. */
let busyNow = false;
let clicks: number[] = [];
let moodTimer: number | undefined;
let blinkTimer: number | undefined;
let glanceTimer: number | undefined;
let napTimer: number | undefined;

const rand = (low: number, high: number): number => low + Math.random() * (high - low);
const live = (): boolean => lively && awake && node !== null;

/** The mascot to draw: the same node until the user picks another. */
export function livelyMascot(mascot: Mascot): SVGSVGElement | null {
  if (mascot !== drawn) {
    drawn = mascot;
    reach = mascotReach(mascot);
    node = mascotSvg(mascot);
    if (node) {
      if (node.querySelector(".m-closed")) node.classList.add("has-closed");
      if (node.querySelector(".m-happy")) node.classList.add("has-happy");
      if (reach.pixel) node.classList.add("pixel");
      const z = document.createElementNS("http://www.w3.org/2000/svg", "text");
      z.setAttribute("class", "m-z");
      z.setAttribute("x", "33");
      z.setAttribute("y", "5");
      z.textContent = "z";
      node.append(z);
      const svg = node;
      svg.addEventListener("animationend", (event) => {
        if (event.target === svg) svg.classList.remove("bounce");
      });
    }
    paint();
    schedule();
  }
  return node;
}

export function setLively(on: boolean): void {
  if (on === lively) return;
  lively = on;
  if (!on) {
    napping = false;
    mood = null;
    aim = [0, 0];
  }
  paint();
  schedule();
}

/** From Rust, only when the direction changes or the screen locks or unlocks. */
export function onGaze(gaze: Gaze): void {
  if (!lively) return;
  const wasAwake = awake;
  awake = gaze.awake;
  if (!awake) {
    schedule();
    return;
  }
  lastMove = Date.now();
  aim = [gaze.x / STEPS_X, gaze.y / STEPS_Y];
  node?.classList.toggle("near", gaze.near);
  if (napping && gaze.near) wakeUp();
  else if (!napping && mood !== "dizzy") look(...aim);
  if (!wasAwake) schedule();
}

/** Agents at work or a notice standing: no nap, and a napping mascot wakes. */
export function setBusy(on: boolean): void {
  busyNow = on;
  if (!on) return;
  lastBusy = Date.now();
  if (napping) wakeUp();
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
  if (clicks.length >= CLICKS_FOR_DIZZY) {
    clicks = [];
    move("wobble");
    feel("dizzy", DIZZY_MS);
    return;
  }
  move("boop");
  feel("happy", 1000);
}

/**
 * The island's color rose: the jump it always had, with eyes that say why
 * when the mascot is lively: a smile for green, wide for the rest, a shake
 * for red.
 */
export function react(tone: "green" | "blue" | "amber" | "red"): void {
  if (!node) return;
  setBusy(true);
  if (lively && tone === "red") move("shake");
  else {
    node.classList.remove("bounce");
    void node.getBoundingClientRect();
    node.classList.add("bounce");
  }
  if (live()) feel(tone === "green" ? "happy" : "wide", 900);
}

function wakeUp(): void {
  napping = false;
  lastMove = Date.now();
  paint();
  look(...aim);
  blink();
  window.setTimeout(blink, 300);
  if (node) {
    node.classList.remove("bounce");
    void node.getBoundingClientRect();
    node.classList.add("bounce");
  }
  schedule();
}

function feel(next: Mood, ms: number): void {
  window.clearTimeout(moodTimer);
  mood = next;
  paint();
  moodTimer = window.setTimeout(() => {
    mood = null;
    paint();
    if (!napping) look(...aim);
  }, ms);
}

function move(kind: Move): void {
  const body = node?.querySelector<SVGGElement>(".m-body");
  if (!body) return;
  body.classList.remove("hop", "boop", "shake", "wobble");
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
  const transform = x === 0 && y === 0 ? "" : `translate(${x.toFixed(2)}px, ${y.toFixed(2)}px)`;
  if (target.style.transform !== transform) target.style.transform = transform;
}

function blink(): void {
  if (!live() || napping || mood) return;
  node?.classList.add("blink");
  window.setTimeout(() => node?.classList.remove("blink"), BLINK_MS);
}

/** The classes that say the mood and the nap; touched only when they change (#51). */
function paint(): void {
  if (!node) return;
  const on = lively;
  for (const name of ["happy", "wide", "dizzy"] as const) node.classList.toggle(name, on && mood === name);
  node.classList.toggle("asleep", on && napping);
  if (!on) {
    node.classList.remove("near", "blink");
    look(0, 0);
  }
}

/** Timers run only while the mascot is lively and the screen unlocked; blinks and looks also not in a nap. */
function schedule(): void {
  window.clearTimeout(blinkTimer);
  window.clearTimeout(glanceTimer);
  window.clearTimeout(napTimer);
  if (!live()) return;
  if (!napping) {
    blinks();
    glances();
  }
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
    if (!napping && now - lastMove >= NAP_MS && now - lastBusy >= NAP_MS) {
      napping = true;
      mood = null;
      look(0, 0.5);
      paint();
      schedule();
      return;
    }
    naps();
  }, NAP_CHECK_MS);
}
