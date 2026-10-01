// Isometric projection for the glass "skyline" scenes. Pure functions so the
// layout maths is testable without a DOM.

export type IsoTone = "accent" | "good" | "warn" | "bad" | "info" | "plasma";

export interface IsoBlock {
  id: string;
  /** Ground-plane position and footprint, in grid units. */
  x: number;
  y: number;
  w: number;
  d: number;
  /** Height in grid units. */
  h: number;
  /** Rows of windows up each face. */
  floors: number;
  /** Fraction of windows lit, 0–1 (e.g. occupancy). */
  lit: number;
  /** Colour of the roof edge and lit windows. */
  tone: IsoTone;
}

export type Point = [number, number];

const COS30 = Math.cos(Math.PI / 6);

export function project(x: number, y: number, z: number, scale: number): Point {
  return [(x - y) * COS30 * scale, ((x + y) / 2 - z) * scale];
}

export interface Window {
  points: Point[];
  lit: boolean;
}

export interface BlockShape {
  block: IsoBlock;
  top: Point[];
  left: Point[];
  right: Point[];
  windows: Window[];
  /** Roof centre, for anchoring labels and tooltips. */
  anchor: Point;
}

/** Deterministic 0–1 hash so a building's lit windows don't flicker per render. */
function hash01(seed: string, n: number): number {
  let h = 2166136261 ^ n;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  h ^= h >>> 13;
  h = Math.imul(h, 0x5bd1e995);
  h ^= h >>> 15;
  return (h >>> 0) / 4294967295;
}

function faceWindows(
  b: IsoBlock,
  scale: number,
  face: "left" | "right",
  offset: number
): Window[] {
  const span = face === "left" ? b.w : b.d;
  const cols = Math.max(1, Math.round(span * 1.6));
  const rows = Math.max(1, b.floors);
  const colW = span / cols;
  const rowH = b.h / rows;
  const ww = colW * 0.42;
  const wh = rowH * 0.42;
  const out: Window[] = [];
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const along = c * colW + (colW - ww) / 2;
      const z = r * rowH + (rowH - wh) / 2;
      const corners: [number, number, number][] =
        face === "left"
          ? [
              [b.x + along, b.y + b.d, z],
              [b.x + along + ww, b.y + b.d, z],
              [b.x + along + ww, b.y + b.d, z + wh],
              [b.x + along, b.y + b.d, z + wh],
            ]
          : [
              [b.x + b.w, b.y + along, z],
              [b.x + b.w, b.y + along + ww, z],
              [b.x + b.w, b.y + along + ww, z + wh],
              [b.x + b.w, b.y + along, z + wh],
            ];
      const idx = offset + r * cols + c;
      out.push({
        points: corners.map(([x, y, zz]) => project(x, y, zz, scale)),
        lit: hash01(b.id, idx) < b.lit,
      });
    }
  }
  return out;
}

export function blockShape(b: IsoBlock, scale: number): BlockShape {
  const p = (x: number, y: number, z: number) => project(x, y, z, scale);
  const { x, y, w, d, h } = b;
  return {
    block: b,
    top: [p(x, y, h), p(x + w, y, h), p(x + w, y + d, h), p(x, y + d, h)],
    left: [
      p(x, y + d, 0),
      p(x + w, y + d, 0),
      p(x + w, y + d, h),
      p(x, y + d, h),
    ],
    right: [
      p(x + w, y, 0),
      p(x + w, y + d, 0),
      p(x + w, y + d, h),
      p(x + w, y, h),
    ],
    windows: [
      ...faceWindows(b, scale, "left", 0),
      ...faceWindows(b, scale, "right", 1000),
    ],
    anchor: p(x + w / 2, y + d / 2, h),
  };
}

/** Back-to-front order so nearer buildings paint over farther ones. */
export function paintOrder(blocks: IsoBlock[]): IsoBlock[] {
  return [...blocks].sort(
    (a, b) => a.x + a.w / 2 + a.y + a.d / 2 - (b.x + b.w / 2 + b.y + b.d / 2)
  );
}

export function bounds(
  points: Point[],
  pad: number
): { x: number; y: number; w: number; h: number } {
  const xs = points.map((q) => q[0]);
  const ys = points.map((q) => q[1]);
  const minX = Math.min(...xs) - pad;
  const minY = Math.min(...ys) - pad;
  return {
    x: minX,
    y: minY,
    w: Math.max(...xs) + pad - minX,
    h: Math.max(...ys) + pad - minY,
  };
}

export const toPath = (pts: Point[]) =>
  `M${pts.map(([a, b]) => `${a.toFixed(1)},${b.toFixed(1)}`).join("L")}Z`;
