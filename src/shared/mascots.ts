/**
 * The mascot gallery. Each mascot is a few shapes on a 40×24 grid, drawn in
 * `currentColor` so the island's state color reaches it. Holes (pupils, eyes
 * on a filled body) use `--mascot-hole`, the color of what is behind.
 * Must match `MASCOTS` in `cortex-core/src/prefs.rs`.
 */

export const MASCOTS = ["eyes", "lens", "owl", "cat", "ghost", "robot", "mustache", "penguin", "none"] as const;
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
  none: "No mascot",
};

/** `f` filled, `s` stroked, `h` a hole. */
type Shape = ["f" | "s" | "h", "rect" | "circle" | "ellipse" | "path", Record<string, string | number>];

const r = (x: number, y: number, width: number, height: number, rx: number): Record<string, number> => ({ x, y, width, height, rx });
const c = (cx: number, cy: number, radius: number): Record<string, number> => ({ cx, cy, r: radius });

const SHAPES: Record<Exclude<Mascot, "none">, Shape[]> = {
  eyes: [["f", "rect", r(13, 6, 5, 12, 2.5)], ["f", "rect", r(22, 6, 5, 12, 2.5)]],
  lens: [["s", "circle", c(20, 12, 8.5)], ["f", "circle", c(20, 12, 3.6)], ["h", "circle", c(22.4, 9.6, 1.1)]],
  owl: [
    ["s", "path", { d: "M7 2l6 4M33 2l-6 4" }],
    ["s", "circle", c(13, 12, 6)],
    ["s", "circle", c(27, 12, 6)],
    ["f", "circle", c(13, 12, 2.3)],
    ["f", "circle", c(27, 12, 2.3)],
    ["f", "path", { d: "M18.3 17h3.4L20 21z" }],
  ],
  cat: [
    ["f", "path", { d: "M8 22V2l7 7h10l7-7v20z" }],
    ["h", "ellipse", { cx: 15.5, cy: 15, rx: 2, ry: 3 }],
    ["h", "ellipse", { cx: 24.5, cy: 15, rx: 2, ry: 3 }],
  ],
  ghost: [
    ["f", "path", { d: "M10 22V11a10 10 0 0 1 20 0v11l-3.3-2.6-3.4 2.6-3.3-2.6-3.3 2.6-3.4-2.6z" }],
    ["h", "circle", c(16.5, 11, 2)],
    ["h", "circle", c(23.5, 11, 2)],
  ],
  robot: [
    ["s", "path", { d: "M20 2.5v4" }],
    ["f", "circle", c(20, 2.4, 1.8)],
    ["s", "rect", r(9, 7, 22, 15, 4)],
    ["f", "rect", r(14, 12, 4, 5, 1.2)],
    ["f", "rect", r(22, 12, 4, 5, 1.2)],
  ],
  mustache: [
    ["s", "circle", c(13, 8, 5)],
    ["s", "circle", c(27, 8, 5)],
    ["s", "path", { d: "M18 8h4" }],
    ["f", "path", { d: "M20 17c-2-3-6-3-9 1 3 3 7 3 9 0 2 3 6 3 9 0-3-4-7-4-9-1z" }],
  ],
  penguin: [
    [
      "f",
      "path",
      {
        d: "M16 1h8v2h-8zM14 3h12v2h-12zM12 5h4v2h-4zM18 5h4v2h-4zM24 5h4v2h-4zM12 7h16v2h-16zM12 9h6v2h-6zM22 9h6v2h-6zM10 11h6v6h-6zM24 11h6v6h-6zM12 17h4v2h-4zM24 17h4v2h-4zM12 19h16v2h-16zM14 21h4v2h-4zM22 21h4v2h-4z",
        "shape-rendering": "crispEdges",
      },
    ],
  ],
};

const SVG = "http://www.w3.org/2000/svg";
const KIND = { f: "m-fill", s: "m-stroke", h: "m-hole" } as const;

/** The mascot as an SVG element, or `null` for "none". */
export function mascotSvg(mascot: Mascot, className = "mascot"): SVGSVGElement | null {
  if (mascot === "none") return null;
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 40 24");
  svg.setAttribute("class", className);
  svg.setAttribute("aria-hidden", "true");
  for (const [kind, tag, attrs] of SHAPES[mascot]) {
    const shape = document.createElementNS(SVG, tag);
    shape.setAttribute("class", KIND[kind]);
    for (const [name, value] of Object.entries(attrs)) shape.setAttribute(name, String(value));
    svg.append(shape);
  }
  return svg;
}
