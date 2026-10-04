// Web Mercator tile math for the plain tile map: fit a set of points, place
// them in pixels, and list the tiles that cover the view.

export const TILE = 256;

export interface LatLng {
  lat: number;
  lng: number;
}

/** World pixel coordinates at a zoom level. */
export function project(p: LatLng, zoom: number): { x: number; y: number } {
  const scale = TILE * 2 ** zoom;
  const lat = Math.max(-85.0511, Math.min(85.0511, p.lat));
  const s = Math.sin((lat * Math.PI) / 180);
  return {
    x: ((p.lng + 180) / 360) * scale,
    y: (0.5 - Math.log((1 + s) / (1 - s)) / (4 * Math.PI)) * scale,
  };
}

/** The highest zoom (up to `max`) at which all points fit in the box. */
export function fitZoom(
  points: LatLng[],
  width: number,
  height: number,
  pad = 48,
  max = 16
): number {
  if (points.length <= 1) return Math.min(max, 13);
  for (let z = max; z >= 1; z--) {
    const xs = points.map((p) => project(p, z));
    const w = Math.max(...xs.map((q) => q.x)) - Math.min(...xs.map((q) => q.x));
    const h = Math.max(...xs.map((q) => q.y)) - Math.min(...xs.map((q) => q.y));
    if (w <= width - pad * 2 && h <= height - pad * 2) return z;
  }
  return 1;
}

export interface View {
  zoom: number;
  /** World pixel at the top-left of the box. */
  originX: number;
  originY: number;
}

/** Centre the points' bounding box in a `width`×`height` box. */
export function fitView(points: LatLng[], width: number, height: number): View {
  const zoom = fitZoom(points, width, height);
  const xs = points.map((p) => project(p, zoom));
  const cx =
    (Math.min(...xs.map((q) => q.x)) + Math.max(...xs.map((q) => q.x))) / 2;
  const cy =
    (Math.min(...xs.map((q) => q.y)) + Math.max(...xs.map((q) => q.y))) / 2;
  return { zoom, originX: cx - width / 2, originY: cy - height / 2 };
}

/** Tiles covering the box, each with where to draw it. */
export function tilesFor(
  view: View,
  width: number,
  height: number
): { z: number; x: number; y: number; left: number; top: number }[] {
  const n = 2 ** view.zoom;
  const x0 = Math.floor(view.originX / TILE);
  const y0 = Math.floor(view.originY / TILE);
  const x1 = Math.floor((view.originX + width) / TILE);
  const y1 = Math.floor((view.originY + height) / TILE);
  const out = [];
  for (let y = y0; y <= y1; y++) {
    if (y < 0 || y >= n) continue;
    for (let x = x0; x <= x1; x++) {
      out.push({
        z: view.zoom,
        x: ((x % n) + n) % n,
        y,
        left: x * TILE - view.originX,
        top: y * TILE - view.originY,
      });
    }
  }
  return out;
}

/** Where a point sits in the box. */
export function place(p: LatLng, view: View): { left: number; top: number } {
  const q = project(p, view.zoom);
  return { left: q.x - view.originX, top: q.y - view.originY };
}
