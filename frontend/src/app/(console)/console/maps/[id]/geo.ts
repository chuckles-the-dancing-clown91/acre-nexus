// Map math for the site map canvas: Web Mercator both ways at any zoom, the
// screen transform for a view, and small geometry helpers.

import { project, TILE } from "@/lib/tilemap";
import type { Geometry } from "@/lib/sitemaps";

export type LngLat = [number, number];

export interface MapView {
  lng: number;
  lat: number;
  /** Fractional zoom. */
  zoom: number;
}

export const MIN_ZOOM = 2;
export const MAX_ZOOM = 22;

export const clampZoom = (z: number) =>
  Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, z));

/** World pixel → longitude and latitude at a zoom. */
export function unproject(x: number, y: number, zoom: number): LngLat {
  const scale = TILE * 2 ** zoom;
  const lng = (x / scale) * 360 - 180;
  const n = Math.PI - (2 * Math.PI * y) / scale;
  const lat = (180 / Math.PI) * Math.atan(Math.sinh(n));
  return [lng, lat];
}

export function world([lng, lat]: LngLat, zoom: number) {
  return project({ lng, lat }, zoom);
}

/** A view's screen transform for a box `w`×`h`. */
export function screen(view: MapView, w: number, h: number) {
  const c = world([view.lng, view.lat], view.zoom);
  const ox = c.x - w / 2;
  const oy = c.y - h / 2;
  return {
    ox,
    oy,
    to(p: LngLat): [number, number] {
      const q = world(p, view.zoom);
      return [q.x - ox, q.y - oy];
    },
    from(sx: number, sy: number): LngLat {
      return unproject(sx + ox, sy + oy, view.zoom);
    },
  };
}

/** Zoom by `dz` keeping the point under (sx, sy) still. */
export function zoomAround(
  view: MapView,
  w: number,
  h: number,
  sx: number,
  sy: number,
  dz: number
): MapView {
  const zoom = clampZoom(view.zoom + dz);
  if (zoom === view.zoom) return view;
  const anchor = screen(view, w, h).from(sx, sy);
  const a = world(anchor, zoom);
  const [lng, lat] = unproject(a.x - (sx - w / 2), a.y - (sy - h / 2), zoom);
  return { lng, lat, zoom };
}

/** Metres a screen pixel covers at a latitude and zoom. */
export function metresPerPixel(lat: number, zoom: number) {
  return (156543.03392 * Math.cos((lat * Math.PI) / 180)) / 2 ** zoom;
}

/** 1, 2 or 5 times a power of ten, at least `v`. */
export function nice(v: number) {
  const p = 10 ** Math.floor(Math.log10(v));
  for (const m of [1, 2, 5, 10]) if (m * p >= v) return m * p;
  return 10 * p;
}

/** Every coordinate in a geometry, flattened. */
export function points(g: Geometry): LngLat[] {
  const out: LngLat[] = [];
  const walk = (c: unknown) => {
    if (!Array.isArray(c)) return;
    if (typeof c[0] === "number" && typeof c[1] === "number")
      out.push([c[0], c[1]]);
    else for (const x of c) walk(x);
  };
  walk(g.coordinates);
  return out;
}

/** The same geometry with every coordinate passed through `fn`. */
export function mapCoords(g: Geometry, fn: (p: LngLat) => LngLat): Geometry {
  const walk = (c: unknown): unknown => {
    if (!Array.isArray(c)) return c;
    if (typeof c[0] === "number" && typeof c[1] === "number")
      return fn([c[0], c[1]]);
    return c.map(walk);
  };
  return { ...g, coordinates: walk(g.coordinates) } as Geometry;
}

/** A label point: a Point itself, else the average of the vertices. */
export function labelAt(g: Geometry): LngLat | null {
  const ps = points(g);
  if (!ps.length) return null;
  if (g.type === "Polygon") {
    const ring = (g.coordinates as LngLat[][])[0] ?? [];
    const open = isClosed(ring) ? ring.slice(0, -1) : ring;
    if (open.length) {
      const s = open.reduce((a, p) => [a[0] + p[0], a[1] + p[1]], [0, 0]);
      return [s[0] / open.length, s[1] / open.length];
    }
  }
  if (g.type === "LineString") {
    const line = g.coordinates as LngLat[];
    return line[Math.floor(line.length / 2)] ?? null;
  }
  const s = ps.reduce((a, p) => [a[0] + p[0], a[1] + p[1]], [0, 0]);
  return [s[0] / ps.length, s[1] / ps.length];
}

export function isClosed(ring: LngLat[]) {
  if (ring.length < 2) return false;
  const a = ring[0];
  const b = ring[ring.length - 1];
  return a[0] === b[0] && a[1] === b[1];
}

/** The editable vertex lists of a shape: each polygon ring (left open) or
 * the line. Null for points and multi-part shapes. */
export function editableRings(g: Geometry): LngLat[][] | null {
  if (g.type === "Polygon")
    return (g.coordinates as LngLat[][]).map((r) =>
      isClosed(r) ? r.slice(0, -1) : r
    );
  if (g.type === "LineString") return [g.coordinates as LngLat[]];
  return null;
}

/** Put edited vertex lists back into the shape, closing polygon rings. */
export function withRings(g: Geometry, rings: LngLat[][]): Geometry {
  if (g.type === "Polygon")
    return {
      type: "Polygon",
      coordinates: rings.map((r) => [...r, r[0]]),
    };
  return { type: "LineString", coordinates: rings[0] };
}

/** An image's natural width and height. */
export function imageSize(src: string): Promise<[number, number]> {
  return new Promise((res, rej) => {
    const img = new Image();
    img.onload = () => res([img.naturalWidth, img.naturalHeight]);
    img.onerror = () => rej(new Error("That file isn't an image"));
    img.src = src;
  });
}
