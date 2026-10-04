"use client";

// A plain tile map: OpenStreetMap tiles under a set of pins, fitted to them.
// No map library; the math is in lib/tilemap.

import { useEffect, useMemo, useRef, useState } from "react";
import { fitView, place, tilesFor, type LatLng } from "@/lib/tilemap";
import { cn } from "@/lib/utils";

export interface Pin extends LatLng {
  id: string;
  label: string;
  tone: "good" | "warn" | "bad" | "neutral";
}

/** Tile source; set NEXT_PUBLIC_MAP_TILES to a provider with a production
 * plan (`{z}`, `{x}`, `{y}` are filled in). OpenStreetMap's own servers are
 * for light use only. */
const TILES =
  process.env.NEXT_PUBLIC_MAP_TILES ||
  "https://tile.openstreetmap.org/{z}/{x}/{y}.png";

function tileUrl(z: number, x: number, y: number) {
  return TILES.replace("{z}", String(z))
    .replace("{x}", String(x))
    .replace("{y}", String(y));
}

const TONE: Record<Pin["tone"], string> = {
  good: "bg-good",
  warn: "bg-warn",
  bad: "bg-bad",
  neutral: "bg-fg-3",
};

export function TileMap({
  pins,
  selected,
  onSelect,
  height = 460,
}: {
  pins: Pin[];
  selected?: string | null;
  onSelect?: (id: string) => void;
  height?: number;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) =>
      setWidth(Math.max(240, e.contentRect.width))
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const view = useMemo(
    () => (pins.length ? fitView(pins, width, height) : null),
    [pins, width, height]
  );
  const tiles = useMemo(
    () => (view ? tilesFor(view, width, height) : []),
    [view, width, height]
  );

  return (
    <div
      ref={box}
      className="relative overflow-hidden rounded-2xl border border-line bg-surface-2"
      style={{ height }}
      role="application"
      aria-label="Map of properties"
    >
      {tiles.map((t) => (
        // eslint-disable-next-line @next/next/no-img-element
        <img
          key={`${t.z}/${t.x}/${t.y}/${t.left}`}
          src={tileUrl(t.z, t.x, t.y)}
          alt=""
          width={256}
          height={256}
          draggable={false}
          onError={(e) => {
            e.currentTarget.style.visibility = "hidden";
          }}
          className="pointer-events-none absolute max-w-none select-none dark:brightness-[0.7] dark:contrast-125 dark:invert dark:hue-rotate-180"
          style={{ left: t.left, top: t.top }}
        />
      ))}
      {view &&
        pins.map((p) => {
          const { left, top } = place(p, view);
          const on = selected === p.id;
          return (
            <button
              key={p.id}
              type="button"
              title={p.label}
              aria-label={p.label}
              aria-pressed={on}
              onClick={() => onSelect?.(p.id)}
              className={cn(
                "absolute -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-md transition",
                TONE[p.tone],
                on
                  ? "z-10 size-6 ring-4 ring-accent/40"
                  : "size-4 hover:scale-125"
              )}
              style={{ left, top }}
            />
          );
        })}
      <div className="absolute right-1 bottom-1 rounded bg-white/80 px-1.5 text-[10px] text-black/70">
        ©{" "}
        <a
          href="https://www.openstreetmap.org/copyright"
          target="_blank"
          rel="noreferrer"
          className="underline"
        >
          OpenStreetMap
        </a>{" "}
        contributors
      </div>
    </div>
  );
}
