/**
 * The pieces of the new island (#77, #78): the state color, the agent tags
 * and the pull request icon. View helpers only; the state comes from Rust.
 */

import { el } from "../shared/dom";
import type { SessionView } from "../shared/ipc";

/** One meaning per color, on the mascot and the border at once. */
export type Tone = "gray" | "green" | "amber" | "red";

const RANK: Record<Tone, number> = { gray: 0, green: 1, amber: 2, red: 3 };

export const sessionKey = (session: SessionView): string => `${session.key.agent}/${session.key.session_id}`;

/**
 * Amber while the agent waits for you, until it moves on; green when it
 * finished and red when it failed, until you click (`seen`). Looking is not
 * enough (#126).
 */
export function sessionTone(session: SessionView, seen: boolean): Tone {
  switch (session.state) {
    case "waiting_approval":
    case "waiting_input":
      return "amber";
    case "finished":
      return seen ? "gray" : "green";
    case "error":
      return seen ? "gray" : "red";
    default:
      return "gray";
  }
}

export function topTone(tones: Iterable<Tone>): Tone {
  let top: Tone = "gray";
  for (const tone of tones) if (RANK[tone] > RANK[top]) top = tone;
  return top;
}

/** Whoever needs you first, then the most recent. */
export function byUrgency(sessions: SessionView[], tone: (s: SessionView) => Tone): SessionView[] {
  return [...sessions].sort((a, b) => RANK[tone(b)] - RANK[tone(a)] || b.last_event_ms - a.last_event_ms);
}

const KNOWN = new Map([["claude-code", "CC"], ["codex", "CX"], ["gemini", "GM"]]);
const PALETTE = 6;

/** "CC" for claude-code, "IAC" for infra-as-code, "MY" for mytool. */
export function agentShort(agent: string): string {
  const known = KNOWN.get(agent);
  if (known) return known;
  const words = agent.split("-").filter(Boolean);
  const short = words.length > 1 ? words.map((w) => w[0]).join("").slice(0, 3) : agent.slice(0, 2);
  return short.toUpperCase();
}

/** A stable color per agent name: known agents have their own, others share a palette. */
export function agentColorClass(agent: string): string {
  if (KNOWN.has(agent)) return `agent-${agent}`;
  let hash = 0;
  for (const ch of agent) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return `agent-c${hash % PALETTE}`;
}

export function agentTag(session: SessionView, tone: Tone, label: string): HTMLElement {
  const node = el("span", `tag ${agentColorClass(session.key.agent)}`);
  node.title = `${session.key.agent} · ${label}`;
  node.append(el("span", "tag-short", agentShort(session.key.agent)), el("span", `tag-dot tone-${tone}`));
  return node;
}

const SVG = "http://www.w3.org/2000/svg";

/** Two branches meeting: the pull request sign. */
export function prIcon(): SVGSVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("class", "pr-icon");
  svg.setAttribute("aria-hidden", "true");
  for (const d of [
    "M4 5.3v5.4",
    "M12 10.7V6a2 2 0 0 0-2-2H7.5",
    "M9 2.5 7.5 4 9 5.5",
    "M5.8 3.5a1.8 1.8 0 1 1-3.6 0 1.8 1.8 0 0 1 3.6 0z",
    "M5.8 12.5a1.8 1.8 0 1 1-3.6 0 1.8 1.8 0 0 1 3.6 0z",
    "M13.8 12.5a1.8 1.8 0 1 1-3.6 0 1.8 1.8 0 0 1 3.6 0z",
  ]) {
    const path = document.createElementNS(SVG, "path");
    path.setAttribute("d", d);
    svg.append(path);
  }
  return svg;
}
