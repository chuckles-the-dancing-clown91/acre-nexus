"use client";

// Renders isometric glass buildings. Decorative when no handlers are passed;
// with `onSelect`, each building becomes a keyboard-reachable control.

import { useMemo } from "react";
import { motion } from "motion/react";
import { cn } from "@/lib/utils";
import {
  blockShape,
  bounds,
  paintOrder,
  project,
  toPath,
  type IsoBlock,
  type IsoTone,
  type Point,
} from "./geometry";

const ROOF: Record<IsoTone, string> = {
  accent: "fill-accent/22 stroke-accent/80",
  good: "fill-good/20 stroke-good/80",
  warn: "fill-warn/20 stroke-warn/80",
  bad: "fill-bad/20 stroke-bad/80",
  info: "fill-info/20 stroke-info/80",
  plasma: "fill-plasma/20 stroke-plasma/80",
};

// Lit windows take the tone's colour; the group's text colour feeds the HUD glow.
const LIT: Record<IsoTone, string> = {
  accent: "fill-accent",
  good: "fill-good",
  warn: "fill-warn",
  bad: "fill-bad",
  info: "fill-info",
  plasma: "fill-plasma",
};
const GLOW: Record<IsoTone, string> = {
  accent: "text-accent",
  good: "text-good",
  warn: "text-warn",
  bad: "text-bad",
  info: "text-info",
  plasma: "text-plasma",
};

export function IsoScene({
  blocks,
  scale = 26,
  activeId,
  onHover,
  onSelect,
  labelFor,
  className,
}: {
  blocks: IsoBlock[];
  scale?: number;
  activeId?: string | null;
  onHover?: (id: string | null) => void;
  onSelect?: (id: string) => void;
  /** Accessible name for an interactive building. */
  labelFor?: (block: IsoBlock) => string;
  className?: string;
}) {
  const { shapes, ground, gridLines, box } = useMemo(() => {
    const ordered = paintOrder(blocks);
    const shapes = ordered.map((b) => blockShape(b, scale));
    const minX = Math.min(...blocks.map((b) => b.x)) - 1;
    const minY = Math.min(...blocks.map((b) => b.y)) - 1;
    const maxX = Math.max(...blocks.map((b) => b.x + b.w)) + 1;
    const maxY = Math.max(...blocks.map((b) => b.y + b.d)) + 1;
    const ground: Point[] = [
      project(minX, minY, 0, scale),
      project(maxX, minY, 0, scale),
      project(maxX, maxY, 0, scale),
      project(minX, maxY, 0, scale),
    ];
    const gridLines: [Point, Point][] = [];
    for (let gx = Math.ceil(minX); gx <= maxX; gx++)
      gridLines.push([
        project(gx, minY, 0, scale),
        project(gx, maxY, 0, scale),
      ]);
    for (let gy = Math.ceil(minY); gy <= maxY; gy++)
      gridLines.push([
        project(minX, gy, 0, scale),
        project(maxX, gy, 0, scale),
      ]);
    const all = [
      ...ground,
      ...shapes.flatMap((s) => [...s.top, ...s.left, ...s.right]),
    ];
    return { shapes, ground, gridLines, box: bounds(all, scale * 0.6) };
  }, [blocks, scale]);

  const interactive = !!onSelect;

  return (
    <svg
      viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`}
      className={cn("h-auto w-full overflow-visible", className)}
      aria-hidden={interactive ? undefined : true}
      role={interactive ? "group" : undefined}
    >
      <defs>
        <radialGradient id="iso-floor" cx="50%" cy="50%" r="50%">
          <stop offset="0%" stopColor="var(--aura)" stopOpacity="0.28" />
          <stop offset="100%" stopColor="var(--aura)" stopOpacity="0" />
        </radialGradient>
      </defs>

      <path d={toPath(ground)} fill="url(#iso-floor)" />
      <g className="stroke-fg/[0.07]" strokeWidth={1}>
        {gridLines.map(([a, b], i) => (
          <line key={i} x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} />
        ))}
      </g>

      {shapes.map((s, i) => {
        const { block } = s;
        const dimmed = activeId != null && activeId !== block.id;
        const active = activeId === block.id;
        return (
          <motion.g
            key={block.id}
            initial={{ opacity: 0, y: 14 }}
            animate={{ opacity: dimmed ? 0.35 : 1, y: 0 }}
            transition={{
              opacity: { duration: 0.35 },
              y: { duration: 0.7, delay: 0.05 * i, ease: [0.22, 1, 0.36, 1] },
            }}
            className={cn(interactive && "cursor-pointer outline-none")}
            tabIndex={interactive ? 0 : undefined}
            role={interactive ? "link" : undefined}
            aria-label={interactive && labelFor ? labelFor(block) : undefined}
            onMouseEnter={() => onHover?.(block.id)}
            onMouseLeave={() => onHover?.(null)}
            onFocus={() => onHover?.(block.id)}
            onBlur={() => onHover?.(null)}
            onClick={() => onSelect?.(block.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect?.(block.id);
              }
            }}
          >
            <path
              d={toPath(s.left)}
              className="fill-fg/[0.045] stroke-fg/15"
              strokeWidth={1}
            />
            <path
              d={toPath(s.right)}
              className="fill-fg/[0.085] stroke-fg/15"
              strokeWidth={1}
            />
            <g
              className={cn(
                GLOW[block.tone],
                "hud:[filter:drop-shadow(0_0_3px_currentColor)]"
              )}
            >
              {s.windows.map((w, wi) => (
                <path
                  key={wi}
                  d={toPath(w.points)}
                  className={
                    w.lit ? cn(LIT[block.tone], "opacity-85") : "fill-fg/[0.07]"
                  }
                />
              ))}
            </g>
            <path
              d={toPath(s.top)}
              className={cn(
                ROOF[block.tone],
                "transition-[stroke-width] duration-200"
              )}
              strokeWidth={active ? 2 : 1.25}
            />
          </motion.g>
        );
      })}
    </svg>
  );
}
