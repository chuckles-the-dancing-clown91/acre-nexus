"use client";

// The site map canvas: map tiles (satellite or streets), a placed plan image
// or a blank grid underneath, the drawn shapes on top in SVG. Drag to pan,
// scroll to zoom. Draws points, shapes and lines, and edits one shape at a
// time: drag a corner, drag a midpoint to add one, Alt-click or right-click a
// corner to remove it, or drag the shape to move it. No map library; the
// tile math is in lib/tilemap.

import { useEffect, useMemo, useRef, useState } from "react";
import { Minus, Plus } from "lucide-react";
import { fitZoom, tilesFor, type LatLng } from "@/lib/tilemap";
import {
  featureColor,
  type Feature,
  type Geometry,
  type SiteMap,
} from "@/lib/sitemaps";
import { cn } from "@/lib/utils";
import {
  clampZoom,
  editableRings,
  labelAt,
  mapCoords,
  metresPerPixel,
  nice,
  points,
  screen,
  unproject,
  withRings,
  world,
  zoomAround,
  type LngLat,
  type MapView,
} from "./geo";

export type Tool =
  { mode: "select" } | { mode: "draw"; shape: "point" | "polygon" | "line" };

const SAT =
  "https://basemap.nationalmap.gov/arcgis/rest/services/USGSImageryOnly/MapServer/tile/{z}/{y}/{x}";
/** Same tile source as the portfolio map; set NEXT_PUBLIC_MAP_TILES for a
 * provider with a production plan. */
const STREETS =
  process.env.NEXT_PUBLIC_MAP_TILES ||
  "https://tile.openstreetmap.org/{z}/{x}/{y}.png";

const LAYERS = {
  satellite: { url: SAT, max: 16, credit: "USGS The National Map" },
  streets: { url: STREETS, max: 19, credit: "© OpenStreetMap contributors" },
} as const;

const tileUrl = (tpl: string, z: number, x: number, y: number) =>
  tpl
    .replace("{z}", String(z))
    .replace("{x}", String(x))
    .replace("{y}", String(y));

type Drag =
  | {
      kind: "pan";
      sx: number;
      sy: number;
      view: MapView;
      moved: boolean;
      hit: string | null;
    }
  | { kind: "vertex"; ring: number; index: number; base: Geometry }
  | { kind: "move"; start: LngLat; base: Geometry };

interface Props {
  map: Pick<
    SiteMap,
    "base_layer" | "center_lng" | "center_lat" | "zoom" | "plan_corners"
  >;
  planUrl: string | null;
  features: Feature[];
  selectedKey: string | null;
  tool: Tool;
  /** The shape being edited, if any. */
  editKey: string | null;
  onSelect: (key: string | null) => void;
  onDrawn: (geometry: Geometry) => void;
  onEdited: (key: string, geometry: Geometry) => void;
  onView?: (lng: number, lat: number, zoom: number) => void;
}

function startView(p: Props): MapView {
  if (p.map.center_lng !== null && p.map.center_lat !== null)
    return {
      lng: p.map.center_lng,
      lat: p.map.center_lat,
      zoom: clampZoom(p.map.zoom || 17),
    };
  const pts = p.features.flatMap((f) => points(f.geometry));
  if (!pts.length) return { lng: -98.5, lat: 39.5, zoom: 3.5 };
  const ll: LatLng[] = pts.map(([lng, lat]) => ({ lng, lat }));
  const zoom = Math.min(19, fitZoom(ll, 800, 560, 48, 20));
  const xs = pts.map((q) => world(q, zoom));
  const cx =
    (Math.min(...xs.map((q) => q.x)) + Math.max(...xs.map((q) => q.x))) / 2;
  const cy =
    (Math.min(...xs.map((q) => q.y)) + Math.max(...xs.map((q) => q.y))) / 2;
  const [lng, lat] = unproject(cx, cy, zoom);
  return { lng, lat, zoom };
}

export function SiteMapCanvas(props: Props) {
  const { map, planUrl, features, selectedKey, tool, editKey } = props;
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 800, h: 560 });
  const [view, setView] = useState<MapView>(() => startView(props));
  const [draft, setDraft] = useState<{ sig: string; pts: LngLat[] }>({
    sig: "",
    pts: [],
  });
  const [hover, setHover] = useState<[number, number] | null>(null);
  const drag = useRef<Drag | null>(null);
  const sig = tool.mode === "draw" ? tool.shape : "";
  const pts = draft.sig === sig ? draft.pts : [];
  const latest = useRef({ props, size, view, pts });
  useEffect(() => {
    latest.current = { props, size, view, pts };
  });
  const drawing = tool.mode === "draw" && tool.shape !== "point";

  // Track the box size.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) =>
      setSize({
        w: Math.max(240, e.contentRect.width),
        h: Math.max(240, e.contentRect.height),
      })
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Wheel zoom needs a non-passive listener to keep the page from scrolling.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      const { w, h } = latest.current.size;
      const sx = e.clientX - r.left;
      const sy = e.clientY - r.top;
      const dz = Math.max(-1, Math.min(1, -e.deltaY * 0.0025));
      setView((v) => zoomAround(v, w, h, sx, sy, dz));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  // Tell the parent where the map is looking.
  useEffect(() => {
    latest.current.props.onView?.(view.lng, view.lat, view.zoom);
  }, [view]);

  const finish = (list: LngLat[]) => {
    const { props: p, view: v, size: sz } = latest.current;
    if (p.tool.mode !== "draw") return;
    // A double-click lands two clicks on about the same spot; drop repeats.
    const sc = screen(v, sz.w, sz.h);
    const clean = list.filter((q, i) => {
      if (i === 0) return true;
      const [x1, y1] = sc.to(q);
      const [x0, y0] = sc.to(list[i - 1]);
      return Math.hypot(x1 - x0, y1 - y0) >= 5;
    });
    if (p.tool.shape === "polygon" && clean.length >= 3)
      p.onDrawn({ type: "Polygon", coordinates: [[...clean, clean[0]]] });
    else if (p.tool.shape === "line" && clean.length >= 2)
      p.onDrawn({ type: "LineString", coordinates: clean });
    else return;
    setDraft({ sig: "", pts: [] });
  };

  // Keys while drawing: Enter finishes, Escape cancels, Backspace undoes.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) return;
      const { props: p } = latest.current;
      if (p.tool.mode !== "draw" || p.tool.shape === "point") return;
      const shape = p.tool.shape;
      if (e.key === "Escape") setDraft({ sig: "", pts: [] });
      else if (e.key === "Enter") finish(latest.current.pts);
      else if (e.key === "Backspace") {
        e.preventDefault();
        setDraft((d) =>
          d.sig === shape ? { sig: d.sig, pts: d.pts.slice(0, -1) } : d
        );
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const s = useMemo(() => screen(view, size.w, size.h), [view, size]);

  const layer =
    map.base_layer === "satellite" || map.base_layer === "streets"
      ? LAYERS[map.base_layer]
      : null;
  const tiles = useMemo(() => {
    if (!layer) return [];
    const tz = Math.max(0, Math.min(layer.max, Math.round(view.zoom)));
    const k = 2 ** (view.zoom - tz);
    const c = world([view.lng, view.lat], tz);
    const v = {
      zoom: tz,
      originX: c.x - size.w / 2 / k,
      originY: c.y - size.h / 2 / k,
    };
    return tilesFor(v, size.w / k, size.h / k).map((t) => ({
      ...t,
      left: t.left * k,
      top: t.top * k,
      size: 256 * k,
    }));
  }, [layer, view, size]);

  const mpp = metresPerPixel(view.lat, view.zoom);
  const local = (e: { clientX: number; clientY: number }): [number, number] => {
    const r = box.current?.getBoundingClientRect();
    return [e.clientX - (r?.left ?? 0), e.clientY - (r?.top ?? 0)];
  };
  const edited = editKey ? features.find((f) => f.key === editKey) : null;

  function click(sx: number, sy: number, hit: string | null) {
    if (tool.mode === "draw") {
      const at = s.from(sx, sy);
      if (tool.shape === "point") {
        props.onDrawn({ type: "Point", coordinates: at });
        return;
      }
      if (tool.shape === "polygon" && pts.length >= 3) {
        const [fx, fy] = s.to(pts[0]);
        if (Math.hypot(fx - sx, fy - sy) < 10) {
          finish(pts);
          return;
        }
      }
      setDraft({ sig, pts: [...pts, at] });
      return;
    }
    if (editKey) return;
    props.onSelect(hit);
  }

  function onPointerDown(e: React.PointerEvent<HTMLDivElement>) {
    if (e.button !== 0) return;
    const [sx, sy] = local(e);
    const t = e.target as Element;
    const vertex = t.closest<SVGElement>("[data-vertex]");
    const mid = t.closest<SVGElement>("[data-mid]");
    const hit = t.closest<SVGElement>("[data-key]")?.dataset.key ?? null;
    e.currentTarget.setPointerCapture(e.pointerId);
    if (edited && vertex) {
      const [ring, index] = vertex.dataset.vertex!.split(":").map(Number);
      const rings = editableRings(edited.geometry);
      if (e.altKey && rings) {
        removeVertex(ring, index);
        return;
      }
      drag.current = { kind: "vertex", ring, index, base: edited.geometry };
      return;
    }
    if (edited && mid) {
      const [ring, index] = mid.dataset.mid!.split(":").map(Number);
      const rings = editableRings(edited.geometry);
      if (!rings) return;
      const next = rings.map((r) => [...r]);
      next[ring].splice(index + 1, 0, s.from(sx, sy));
      const g = withRings(edited.geometry, next);
      props.onEdited(edited.key, g);
      drag.current = { kind: "vertex", ring, index: index + 1, base: g };
      return;
    }
    if (edited && hit === edited.key) {
      drag.current = {
        kind: "move",
        start: s.from(sx, sy),
        base: edited.geometry,
      };
      return;
    }
    drag.current = { kind: "pan", sx, sy, view, moved: false, hit };
  }

  function removeVertex(ring: number, index: number) {
    if (!edited) return;
    const rings = editableRings(edited.geometry);
    if (!rings) return;
    const min = edited.geometry.type === "Polygon" ? 3 : 2;
    if (rings[ring].length <= min) return;
    const next = rings.map((r, i) =>
      i === ring ? r.filter((_, j) => j !== index) : r
    );
    props.onEdited(edited.key, withRings(edited.geometry, next));
  }

  function onPointerMove(e: React.PointerEvent<HTMLDivElement>) {
    const [sx, sy] = local(e);
    if (drawing) setHover([sx, sy]);
    const d = drag.current;
    if (!d) return;
    if (d.kind === "pan") {
      const dx = sx - d.sx;
      const dy = sy - d.sy;
      if (!d.moved && Math.hypot(dx, dy) < 4) return;
      d.moved = true;
      const c = world([d.view.lng, d.view.lat], d.view.zoom);
      const [lng, lat] = unproject(c.x - dx, c.y - dy, d.view.zoom);
      setView({ lng, lat, zoom: d.view.zoom });
      return;
    }
    if (!editKey) return;
    const at = s.from(sx, sy);
    if (d.kind === "vertex") {
      if (d.base.type === "Point") {
        props.onEdited(editKey, { type: "Point", coordinates: at });
        return;
      }
      const rings = editableRings(d.base);
      if (!rings) return;
      const next = rings.map((r) => [...r]);
      next[d.ring][d.index] = at;
      props.onEdited(editKey, withRings(d.base, next));
      return;
    }
    const dl = at[0] - d.start[0];
    const dt = at[1] - d.start[1];
    props.onEdited(
      editKey,
      mapCoords(d.base, ([x, y]) => [x + dl, y + dt])
    );
  }

  function onPointerUp(e: React.PointerEvent<HTMLDivElement>) {
    const d = drag.current;
    drag.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId))
      e.currentTarget.releasePointerCapture(e.pointerId);
    if (d?.kind === "pan" && !d.moved) {
      const [sx, sy] = local(e);
      click(sx, sy, d.hit);
    }
  }

  function onDoubleClick(e: React.MouseEvent<HTMLDivElement>) {
    e.preventDefault();
    if (drawing) {
      finish(pts);
      return;
    }
    if (tool.mode === "select" && !editKey) {
      const [sx, sy] = local(e);
      setView((v) => zoomAround(v, size.w, size.h, sx, sy, 1));
    }
  }

  function onContextMenu(e: React.MouseEvent<HTMLDivElement>) {
    const vertex = (e.target as Element).closest<SVGElement>("[data-vertex]");
    if (!edited || !vertex) return;
    e.preventDefault();
    const [ring, index] = vertex.dataset.vertex!.split(":").map(Number);
    removeVertex(ring, index);
  }

  const zoomBy = (dz: number) =>
    setView((v) => zoomAround(v, size.w, size.h, size.w / 2, size.h / 2, dz));

  // Scale bar: a round number of feet near 100 pixels.
  const feet = nice(100 * mpp * 3.28084);
  const barPx = feet / 3.28084 / mpp;

  return (
    <div
      ref={box}
      role="application"
      aria-label="Site map"
      className={cn(
        "relative h-full w-full touch-none overflow-hidden select-none",
        map.base_layer === "satellite" ? "bg-[#1d2a1f]" : "bg-[#f4f1ea]",
        tool.mode === "draw"
          ? "cursor-crosshair"
          : editKey
            ? "cursor-default"
            : "cursor-grab active:cursor-grabbing"
      )}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => (drag.current = null)}
      onPointerLeave={() => setHover(null)}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
    >
      {layer &&
        tiles.map((t) => (
          // eslint-disable-next-line @next/next/no-img-element -- map tiles
          <img
            key={`${t.z}/${t.x}/${t.y}/${Math.round(t.left)}`}
            src={tileUrl(layer.url, t.z, t.x, t.y)}
            alt=""
            draggable={false}
            onError={(e) => {
              e.currentTarget.style.visibility = "hidden";
            }}
            className={cn(
              "pointer-events-none absolute max-w-none",
              map.base_layer === "streets" &&
                "dark:brightness-[0.7] dark:contrast-125 dark:hue-rotate-180 dark:invert"
            )}
            style={{
              left: t.left,
              top: t.top,
              width: Math.ceil(t.size) + 0.5,
              height: Math.ceil(t.size) + 0.5,
            }}
          />
        ))}

      <svg
        className="absolute inset-0"
        width={size.w}
        height={size.h}
        viewBox={`0 0 ${size.w} ${size.h}`}
      >
        {map.base_layer === "grid" && <Grid s={s} mpp={mpp} size={size} />}
        {map.base_layer === "plan" &&
          planUrl &&
          map.plan_corners?.length === 4 && (
            <PlanImage url={planUrl} corners={map.plan_corners} s={s} />
          )}

        {features.map((f) => (
          <Shape
            key={f.key}
            f={f}
            s={s}
            selected={f.key === selectedKey || f.key === editKey}
          />
        ))}

        {view.zoom >= 15 &&
          features.map((f) => {
            if (!f.name) return null;
            const at = labelAt(f.geometry);
            if (!at) return null;
            const [x, y] = s.to(at);
            return (
              <text
                key={`l-${f.key}`}
                x={x}
                y={f.geometry.type === "Point" ? y + 18 : y + 4}
                textAnchor="middle"
                className="pointer-events-none fill-[#111827] text-[11px] font-medium"
                stroke="#ffffff"
                strokeWidth={3}
                paintOrder="stroke"
              >
                {f.name}
              </text>
            );
          })}

        {edited && <Handles f={edited} s={s} />}

        {drawing && pts.length > 0 && (
          <Draft
            pts={pts.map((p) => s.to(p))}
            hover={hover}
            closed={tool.mode === "draw" && tool.shape === "polygon"}
          />
        )}
      </svg>

      <div className="absolute top-3 right-3 flex flex-col overflow-hidden rounded-xl border border-black/10 bg-white/90 text-[#111827] shadow-sm">
        <button
          type="button"
          aria-label="Zoom in"
          onPointerDown={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
          onClick={() => zoomBy(1)}
          className="flex size-8 items-center justify-center hover:bg-black/5"
        >
          <Plus className="size-4" />
        </button>
        <button
          type="button"
          aria-label="Zoom out"
          onPointerDown={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
          onClick={() => zoomBy(-1)}
          className="flex size-8 items-center justify-center border-t border-black/10 hover:bg-black/5"
        >
          <Minus className="size-4" />
        </button>
      </div>

      <div className="pointer-events-none absolute bottom-2 left-2 rounded bg-white/80 px-1.5 py-0.5 text-[10px] text-black/70">
        <div
          className="mb-0.5 h-1 border-x border-b border-black/60"
          style={{ width: barPx }}
        />
        {feet >= 5280 ? `${feet / 5280} mi` : `${feet.toLocaleString()} ft`}
      </div>
      {layer && (
        <div className="absolute right-1 bottom-1 rounded bg-white/80 px-1.5 text-[10px] text-black/70">
          {layer.credit}
        </div>
      )}
    </div>
  );
}

type Screen = ReturnType<typeof screen>;

function pathOf(rings: LngLat[][], s: Screen, close: boolean) {
  return rings
    .map(
      (r) =>
        r
          .map((p, i) => {
            const [x, y] = s.to(p);
            return `${i ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`;
          })
          .join("") + (close ? "Z" : "")
    )
    .join("");
}

function Shape({
  f,
  s,
  selected,
}: {
  f: Feature;
  s: Screen;
  selected: boolean;
}) {
  const color = featureColor(f);
  const g = f.geometry;
  const common = { "data-key": f.key, className: "cursor-pointer" };
  if (g.type === "Point" || g.type === "MultiPoint") {
    const ps =
      g.type === "Point"
        ? [g.coordinates as LngLat]
        : (g.coordinates as LngLat[]);
    return (
      <g {...common}>
        {ps.map((p, i) => {
          const [x, y] = s.to(p);
          return (
            <circle
              key={i}
              cx={x}
              cy={y}
              r={selected ? 9 : 6}
              fill={color}
              stroke="#ffffff"
              strokeWidth={2}
            />
          );
        })}
      </g>
    );
  }
  if (g.type === "LineString" || g.type === "MultiLineString") {
    const lines =
      g.type === "LineString"
        ? [g.coordinates as LngLat[]]
        : (g.coordinates as LngLat[][]);
    const d = pathOf(lines, s, false);
    return (
      <g {...common}>
        {/* A wide invisible stroke makes thin lines easy to pick. */}
        <path d={d} fill="none" stroke="transparent" strokeWidth={14} />
        <path
          d={d}
          fill="none"
          stroke={color}
          strokeWidth={selected ? 5 : f.kind === "road" ? 4 : 2}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </g>
    );
  }
  if (g.type === "Polygon" || g.type === "MultiPolygon") {
    const polys =
      g.type === "Polygon"
        ? [g.coordinates as LngLat[][]]
        : (g.coordinates as LngLat[][][]);
    const d = polys.map((rings) => pathOf(rings, s, true)).join("");
    return (
      <path
        {...common}
        d={d}
        fill={color}
        fillOpacity={selected ? 0.55 : f.kind === "boundary" ? 0.06 : 0.32}
        fillRule="evenodd"
        stroke={color}
        strokeWidth={selected ? 3.5 : 2}
        strokeLinejoin="round"
      />
    );
  }
  return null;
}

/** Corner handles and midpoints for the shape being edited. */
function Handles({ f, s }: { f: Feature; s: Screen }) {
  const g = f.geometry;
  if (g.type === "Point") {
    const [x, y] = s.to(g.coordinates as LngLat);
    return (
      <circle
        data-vertex="-1:0"
        cx={x}
        cy={y}
        r={8}
        fill="#ffffff"
        stroke="#0e7c86"
        strokeWidth={3}
        className="cursor-move"
      />
    );
  }
  const rings = editableRings(g);
  if (!rings) return null;
  const closed = g.type === "Polygon";
  return (
    <g>
      {rings.map((r, ri) =>
        r.map((p, i) => {
          if (!closed && i === r.length - 1) return null;
          const q = r[(i + 1) % r.length];
          const [x1, y1] = s.to(p);
          const [x2, y2] = s.to(q);
          return (
            <circle
              key={`m${ri}:${i}`}
              data-mid={`${ri}:${i}`}
              cx={(x1 + x2) / 2}
              cy={(y1 + y2) / 2}
              r={4}
              fill="#ffffff"
              fillOpacity={0.75}
              stroke="#0e7c86"
              strokeWidth={1.5}
              className="cursor-copy"
            />
          );
        })
      )}
      {rings.map((r, ri) =>
        r.map((p, i) => {
          const [x, y] = s.to(p);
          return (
            <circle
              key={`v${ri}:${i}`}
              data-vertex={`${ri}:${i}`}
              cx={x}
              cy={y}
              r={6}
              fill="#0e7c86"
              stroke="#ffffff"
              strokeWidth={2}
              className="cursor-move"
            />
          );
        })
      )}
    </g>
  );
}

function Draft({
  pts,
  hover,
  closed,
}: {
  pts: [number, number][];
  hover: [number, number] | null;
  closed: boolean;
}) {
  const all = hover ? [...pts, hover] : pts;
  const d =
    all.map(([x, y], i) => `${i ? "L" : "M"}${x} ${y}`).join("") +
    (closed && all.length > 2 ? "Z" : "");
  return (
    <g className="pointer-events-none">
      <path
        d={d}
        fill={closed ? "#0e7c86" : "none"}
        fillOpacity={0.18}
        stroke="#0e7c86"
        strokeWidth={2}
        strokeDasharray="6 4"
      />
      {pts.map(([x, y], i) => (
        <circle
          key={i}
          cx={x}
          cy={y}
          r={i === 0 && closed ? 6 : 4}
          fill="#ffffff"
          stroke="#0e7c86"
          strokeWidth={2}
        />
      ))}
    </g>
  );
}

function PlanImage({
  url,
  corners,
  s,
}: {
  url: string;
  corners: [number, number][];
  s: Screen;
}) {
  // Corners run top-left, top-right, bottom-right, bottom-left.
  const [tl, tr, , bl] = corners.map((c) => s.to(c as LngLat));
  const m = [
    tr[0] - tl[0],
    tr[1] - tl[1],
    bl[0] - tl[0],
    bl[1] - tl[1],
    tl[0],
    tl[1],
  ];
  return (
    <image
      href={url}
      width={1}
      height={1}
      preserveAspectRatio="none"
      transform={`matrix(${m.join(" ")})`}
      opacity={0.92}
      className="pointer-events-none"
    />
  );
}

function Grid({
  s,
  mpp,
  size,
}: {
  s: Screen;
  mpp: number;
  size: { w: number; h: number };
}) {
  const step = nice(28 * mpp);
  const px = step / mpp;
  const x0 = -(((s.ox % px) + px) % px);
  const y0 = -(((s.oy % px) + px) % px);
  const xs: number[] = [];
  const ys: number[] = [];
  for (let x = x0; x < size.w; x += px) xs.push(x);
  for (let y = y0; y < size.h; y += px) ys.push(y);
  return (
    <g className="pointer-events-none" stroke="#d8d2c4" strokeWidth={1}>
      {xs.map((x) => (
        <line key={`x${x}`} x1={x} x2={x} y1={0} y2={size.h} />
      ))}
      {ys.map((y) => (
        <line key={`y${y}`} x1={0} x2={size.w} y1={y} y2={y} />
      ))}
    </g>
  );
}
