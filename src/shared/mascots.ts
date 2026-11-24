/**
 * The mascot gallery. Each mascot is a few shapes on a 40×24 grid, drawn in
 * `currentColor` so the island's state color reaches it. Holes (pupils, eyes
 * on a filled body) use `--mascot-hole`, the color of what is behind.
 * Must match `MASCOTS` in `cortex-core/src/prefs.rs`.
 */

export const MASCOTS = [
  "eyes", "lens", "owl", "cat", "ghost", "robot", "mustache", "penguin", "skull", "ninja", "bunny", "samurai", "none",
] as const;
export type Mascot = (typeof MASCOTS)[number];

export const MASCOT_NAMES: Record<Mascot, string> = {
  eyes: "Eyes",
  lens: "Lens",
  owl: "Owl",
  cat: "Cat",
  ghost: "Ghost",
  robot: "Robot",
  mustache: "Glasses and mustache",
  penguin: "Pixel penguin",
  skull: "Skull",
  ninja: "Ninja",
  bunny: "Bunny mochi",
  samurai: "Samurai mochi",
  none: "No mascot",
};

/** `f` filled, `s` stroked, `t` thinly stroked, `h` a hole, `k` a line in the hole's color. */
type Shape = ["f" | "s" | "t" | "h" | "k", "rect" | "circle" | "ellipse" | "path", Record<string, string | number>];

/** A part that moves on its own (#145): eyebrows, the mustache, ears, the bun. */
type Part = "brow" | "stache" | "ear-l" | "ear-r" | "knot";

interface Design {
  shapes: Shape[];
  /** The shapes that follow the mouse (consecutive indexes); without them the whole head turns. */
  look?: number[];
  /** The shapes that close when the eyes do. */
  lids?: number[];
  /** Drawn in place of the lids when the eyes close or smile. */
  closed?: Shape[];
  happy?: Shape[];
  /** Consecutive shapes of one part move as one group (an ear with its inner ear). */
  parts?: Record<number, Part>;
  /** Pixel art: the eyes move a whole pixel at a time. */
  pixel?: true;
  /** How far the eyes go, in grid units across and down. */
  reach?: [number, number];
}

const r = (x: number, y: number, width: number, height: number, rx: number): Record<string, number> => ({ x, y, width, height, rx });
const c = (cx: number, cy: number, radius: number): Record<string, number> => ({ cx, cy, r: radius });

const DESIGNS: Record<Exclude<Mascot, "none">, Design> = {
  eyes: { shapes: [["f", "rect", r(13, 6, 5, 12, 2.5)], ["f", "rect", r(22, 6, 5, 12, 2.5)]], look: [0, 1], lids: [0, 1] },
  lens: {
    shapes: [["s", "circle", c(20, 12, 8.5)], ["f", "circle", c(20, 12, 3.6)], ["h", "circle", c(22.4, 9.6, 1.1)]],
    look: [1, 2],
    lids: [1, 2],
  },
  owl: {
    shapes: [
      ["s", "path", { d: "M7 2l6 4M33 2l-6 4" }],
      ["s", "circle", c(13, 12, 6)],
      ["s", "circle", c(27, 12, 6)],
      ["f", "circle", c(13, 12, 2.3)],
      ["f", "circle", c(27, 12, 2.3)],
      ["f", "path", { d: "M18.3 17h3.4L20 21z" }],
    ],
    look: [3, 4],
    lids: [1, 2, 3, 4],
    // A flattened ring reads as eyes turned sideways: the lids draw a curve instead.
    closed: [["s", "path", { d: "M7.5 12.5q5.5 4.2 11 0M21.5 12.5q5.5 4.2 11 0" }]],
    happy: [["s", "path", { d: "M8 14q5-5.5 10 0M22 14q5-5.5 10 0" }]],
  },
  cat: {
    shapes: [
      ["f", "path", { d: "M8 22V2l7 7h10l7-7v20z" }],
      ["h", "ellipse", { cx: 15.5, cy: 15, rx: 2, ry: 3 }],
      ["h", "ellipse", { cx: 24.5, cy: 15, rx: 2, ry: 3 }],
    ],
    look: [1, 2],
    lids: [1, 2],
  },
  ghost: {
    shapes: [
      ["f", "path", { d: "M10 22V11a10 10 0 0 1 20 0v11l-3.3-2.6-3.4 2.6-3.3-2.6-3.3 2.6-3.4-2.6z" }],
      ["h", "circle", c(16.5, 11, 2)],
      ["h", "circle", c(23.5, 11, 2)],
    ],
    look: [1, 2],
    lids: [1, 2],
  },
  robot: {
    shapes: [
      ["s", "path", { d: "M20 2.5v4" }],
      ["f", "circle", c(20, 2.4, 1.8)],
      ["s", "rect", r(9, 7, 22, 15, 4)],
      ["f", "rect", r(14, 12, 4, 5, 1.2)],
      ["f", "rect", r(22, 12, 4, 5, 1.2)],
    ],
    look: [3, 4],
    lids: [3, 4],
  },
  mustache: {
    shapes: [
      ["s", "circle", c(13, 10, 5)],
      ["s", "circle", c(27, 10, 5)],
      ["s", "path", { d: "M18 10h4" }],
      ["f", "circle", c(13, 10, 1.8)],
      ["f", "circle", c(27, 10, 1.8)],
      ["s", "path", { d: "M9 3.4q4-2.2 8 0" }],
      ["s", "path", { d: "M23 3.4q4-2.2 8 0" }],
      ["f", "path", { d: "M20 19.5c-2-3-6-3-9 1 3 3 7 3 9 0 2 3 6 3 9 0-3-4-7-4-9-1z" }],
    ],
    look: [3, 4],
    lids: [3, 4],
    parts: { 5: "brow", 6: "brow", 7: "stache" },
    reach: [1.9, 1.4],
  },
  penguin: {
    shapes: [
      [
        "f",
        "path",
        {
          // The eye row is whole: the eyes are the two holes below, free to move.
          d: "M16 1h8v2h-8zM14 3h12v2h-12zM12 5h16v2h-16zM12 7h16v2h-16zM12 9h6v2h-6zM22 9h6v2h-6zM10 11h6v6h-6zM24 11h6v6h-6zM12 17h4v2h-4zM24 17h4v2h-4zM12 19h16v2h-16zM14 21h4v2h-4zM22 21h4v2h-4z",
          "shape-rendering": "crispEdges",
        },
      ],
      ["h", "rect", { x: 16, y: 5, width: 2, height: 2, "shape-rendering": "crispEdges" }],
      ["h", "rect", { x: 22, y: 5, width: 2, height: 2, "shape-rendering": "crispEdges" }],
    ],
    look: [1, 2],
    lids: [1, 2],
    pixel: true,
  },
  skull: {
    shapes: [
      ["f", "path", { d: "M20 1.5C13.6 1.5 9.5 5.8 9.5 11.1c0 3.1 1.4 5.4 3.6 6.8V21a1.5 1.5 0 0 0 1.5 1.5h10.8a1.5 1.5 0 0 0 1.5-1.5v-3.1c2.2-1.4 3.6-3.7 3.6-6.8 0-5.3-4.1-9.6-10.5-9.6z" }],
      ["h", "ellipse", { cx: 15.6, cy: 11.3, rx: 2.9, ry: 3.1 }],
      ["h", "ellipse", { cx: 24.4, cy: 11.3, rx: 2.9, ry: 3.1 }],
      ["f", "circle", c(15.6, 11.5, 1.15)],
      ["f", "circle", c(24.4, 11.5, 1.15)],
      ["h", "path", { d: "M20 14.2l-1.4 2.6h2.8z" }],
      ["k", "path", { d: "M17.2 19.3v3.2M20 19.3v3.2M22.8 19.3v3.2" }],
    ],
    look: [3, 4],
    lids: [3, 4],
    reach: [1.3, 1.3],
  },
  ninja: {
    shapes: [
      ["f", "circle", c(20, 12, 10.3)],
      ["h", "rect", r(8.8, 8.4, 22.4, 6.6, 3.2)],
      ["f", "ellipse", { cx: 16, cy: 12.3, rx: 2.1, ry: 1.35 }],
      ["f", "ellipse", { cx: 24, cy: 12.3, rx: 2.1, ry: 1.35 }],
      ["t", "path", { d: "M13 9.9l4.2.9" }],
      ["t", "path", { d: "M27 9.9l-4.2.9" }],
      ["s", "path", { d: "M29.4 6.4l5.6-3.4M29.8 8l5.8-.8" }],
    ],
    look: [2, 3],
    lids: [2, 3],
    parts: { 4: "brow", 5: "brow" },
    reach: [1.4, 0.8],
  },
  bunny: {
    shapes: [
      // Ellipses tilted inside the path, so a rotation of the ear never fights a transform attribute.
      ["f", "path", { d: "M14.32 1.26A2.3 5.3 -14 0 1 16.88 11.54A2.3 5.3 -14 0 1 14.32 1.26z" }],
      ["h", "path", { d: "M14.8 3A.8 3.3 -14 0 1 16.4 9.4A.8 3.3 -14 0 1 14.8 3z" }],
      ["f", "path", { d: "M25.68 1.26A2.3 5.3 14 0 1 23.12 11.54A2.3 5.3 14 0 1 25.68 1.26z" }],
      ["h", "path", { d: "M25.2 3A.8 3.3 14 0 1 23.6 9.4A.8 3.3 14 0 1 25.2 3z" }],
      ["f", "ellipse", { cx: 20, cy: 15.6, rx: 9.6, ry: 7.4 }],
      ["h", "circle", c(16.6, 15.2, 1.05)],
      ["h", "circle", c(23.4, 15.2, 1.05)],
      ["k", "path", { d: "M18.6 17.6q.7.9 1.4 0q.7.9 1.4 0" }],
    ],
    look: [5, 6],
    lids: [5, 6],
    parts: { 0: "ear-l", 1: "ear-l", 2: "ear-r", 3: "ear-r" },
    reach: [1.2, 0.8],
  },
  samurai: {
    shapes: [
      ["f", "ellipse", { cx: 20, cy: 14.8, rx: 9.6, ry: 8 }],
      ["f", "circle", c(20, 4.9, 2.4)],
      ["k", "path", { d: "M17.9 7h4.2" }],
      ["h", "rect", r(10.4, 9.6, 19.2, 2.4, 1.2)],
      ["s", "path", { d: "M28.4 10.3l5.2-2.6M28.7 11.5l5.4 1" }],
      ["k", "path", { d: "M14.6 13.4l2.9.8" }],
      ["k", "path", { d: "M25.4 13.4l-2.9.8" }],
      ["h", "circle", c(17, 15.8, 1.1)],
      ["h", "circle", c(23, 15.8, 1.1)],
      ["k", "path", { d: "M19 18.8h2" }],
    ],
    look: [7, 8],
    lids: [7, 8],
    parts: { 1: "knot", 2: "knot", 5: "brow", 6: "brow" },
    reach: [1.2, 0.8],
  },
};

const SVG = "http://www.w3.org/2000/svg";
const KIND = { f: "m-fill", s: "m-stroke", t: "m-stroke-thin", h: "m-hole", k: "m-hole-line" } as const;

function shapeNode([kind, tag, attrs]: Shape, extra?: string): SVGElement {
  const shape = document.createElementNS(SVG, tag);
  shape.setAttribute("class", extra ? `${KIND[kind]} ${extra}` : KIND[kind]);
  for (const [name, value] of Object.entries(attrs)) shape.setAttribute(name, String(value));
  return shape;
}

function group(className: string): SVGGElement {
  const g = document.createElementNS(SVG, "g");
  g.setAttribute("class", className);
  return g;
}

/**
 * The mascot as an SVG element, or `null` for "none". Its pieces are named for
 * the lively mascot (#145): `m-body` jumps, `m-head` turns, `m-look` holds the
 * eyes that follow the mouse, `m-lid` closes, `m-closed` and `m-happy` replace
 * the lids where flattening them would not read.
 */
export function mascotSvg(mascot: Mascot, className = "mascot"): SVGSVGElement | null {
  if (mascot === "none") return null;
  const design = DESIGNS[mascot];
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 40 24");
  svg.setAttribute("class", className);
  svg.setAttribute("aria-hidden", "true");
  const body = group("m-body");
  const head = group("m-head");
  body.append(head);
  svg.append(body);
  let look: SVGGElement | null = null;
  let part: { name: Part; g: SVGGElement } | null = null;
  design.shapes.forEach((shape, i) => {
    const node = shapeNode(shape, design.lids?.includes(i) ? "m-lid" : undefined);
    const name = design.parts?.[i];
    if (design.look?.includes(i)) {
      if (!look) {
        look = group("m-look");
        head.append(look);
      }
      look.append(node);
    } else if (name) {
      if (part?.name !== name) {
        part = { name, g: group(`m-${name}`) };
        head.append(part.g);
      }
      part.g.append(node);
    } else head.append(node);
    if (name !== part?.name) part = null;
  });
  for (const alt of ["closed", "happy"] as const) {
    const shapes = design[alt];
    if (!shapes) continue;
    const g = group(`m-alt m-${alt}`);
    for (const shape of shapes) g.append(shapeNode(shape));
    head.append(g);
  }
  return svg;
}

/** How a mascot's eyes move (#145): how far, in grid units, and by whole pixels or not. */
export function mascotReach(mascot: Mascot): { reach: [number, number]; pixel: boolean } {
  if (mascot === "none") return { reach: [0, 0], pixel: false };
  const design = DESIGNS[mascot];
  const look = Boolean(design.look?.length);
  return { reach: design.reach ?? (look ? [2.2, 1.6] : [1.1, 0.8]), pixel: Boolean(design.pixel) };
}
