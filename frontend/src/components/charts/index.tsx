"use client";

// Glass-native charts: a trend card (area or bars, with hover read-out), a
// sparkline, and a ring gauge. Hand-rolled SVG so they share the theme's
// tokens exactly and stay tiny.

import { useId, useMemo, useState } from "react";
import { motion } from "motion/react";
import { monthLabel, niceCeil } from "@/lib/chart";
import { cn } from "@/lib/utils";
import type { Tone } from "@/components/ui/badge";

const STROKE: Record<Tone, string> = {
  neutral: "var(--fg-3)",
  accent: "var(--accent)",
  good: "var(--good)",
  warn: "var(--warn)",
  bad: "var(--bad)",
  info: "var(--info)",
  plasma: "var(--plasma)",
};

const W = 320;

function points(values: number[], h: number, pad = 6) {
  const ceil = niceCeil(Math.max(...values, 0));
  const step = values.length > 1 ? W / (values.length - 1) : 0;
  return values.map(
    (v, i) =>
      [i * step, pad + (h - pad * 2) * (1 - Math.max(v, 0) / ceil)] as const
  );
}

function smoothPath(pts: readonly (readonly [number, number])[]): string {
  if (pts.length === 0) return "";
  let d = `M${pts[0][0]},${pts[0][1]}`;
  for (let i = 1; i < pts.length; i++) {
    const [x0, y0] = pts[i - 1];
    const [x1, y1] = pts[i];
    const cx = (x0 + x1) / 2;
    d += ` C${cx},${y0} ${cx},${y1} ${x1},${y1}`;
  }
  return d;
}

export function Sparkline({
  values,
  tone = "accent",
  height = 36,
  className,
}: {
  values: number[];
  tone?: Tone;
  height?: number;
  className?: string;
}) {
  const id = useId();
  if (values.length < 2) return null;
  const pts = points(values, height, 3);
  const line = smoothPath(pts);
  return (
    <svg
      viewBox={`0 0 ${W} ${height}`}
      preserveAspectRatio="none"
      className={cn("w-full", className)}
      style={{ height }}
      aria-hidden
    >
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={STROKE[tone]} stopOpacity={0.28} />
          <stop offset="100%" stopColor={STROKE[tone]} stopOpacity={0} />
        </linearGradient>
      </defs>
      <path d={`${line} L${W},${height} L0,${height} Z`} fill={`url(#${id})`} />
      <path
        d={line}
        fill="none"
        stroke={STROKE[tone]}
        strokeWidth={1.75}
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

export function Ring({
  value,
  tone = "accent",
  size = 56,
  stroke = 6,
  children,
}: {
  /** 0–100 */
  value: number;
  tone?: Tone;
  size?: number;
  stroke?: number;
  children?: React.ReactNode;
}) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const pct = Math.min(100, Math.max(0, value));
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90" aria-hidden>
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke="var(--fill-2)"
          strokeWidth={stroke}
        />
        <motion.circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke={STROKE[tone]}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          initial={{ strokeDashoffset: c }}
          animate={{ strokeDashoffset: c * (1 - pct / 100) }}
          transition={{ duration: 1.1, ease: [0.22, 1, 0.36, 1] }}
          style={{ filter: `drop-shadow(0 0 6px ${STROKE[tone]})` }}
        />
      </svg>
      {children && (
        <div className="absolute inset-0 flex items-center justify-center">
          {children}
        </div>
      )}
    </div>
  );
}

export function TrendChart({
  title,
  months,
  values,
  format,
  kind = "area",
  tone = "accent",
  invert = false,
  height = 120,
}: {
  title: string;
  months: string[];
  values: number[];
  format: (v: number) => string;
  kind?: "area" | "bar";
  tone?: Tone;
  /** Lower is better (delinquency): a rise reads as bad. */
  invert?: boolean;
  height?: number;
}) {
  const id = useId();
  const [hover, setHover] = useState<number | null>(null);
  const last = values.length - 1;
  const idx = hover ?? last;
  const prev = idx > 0 ? values[idx - 1] : null;
  const delta =
    prev !== null && prev !== 0 ? (values[idx] - prev) / Math.abs(prev) : null;
  const deltaGood = delta === null ? null : invert ? delta <= 0 : delta >= 0;

  const { pts, ceil } = useMemo(
    () => ({
      pts: points(values, height),
      ceil: niceCeil(Math.max(...values, 0)),
    }),
    [values, height]
  );
  const line = smoothPath(pts);
  const slot = W / Math.max(values.length, 1);

  return (
    <div className="glass relative rounded-2xl p-5">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="eyebrow">{title}</div>
          <div className="figure mt-1.5 text-[26px] leading-none font-semibold text-fg">
            {format(values[idx] ?? 0)}
          </div>
        </div>
        <div className="text-right">
          <div className="text-[11px] text-fg-3">
            {monthLabel(months[idx] ?? "")}
          </div>
          {delta !== null && Number.isFinite(delta) && (
            <div
              className={cn(
                "mt-1 font-mono text-[11px] font-medium",
                deltaGood ? "text-good" : "text-bad"
              )}
            >
              {delta >= 0 ? "▲" : "▼"} {Math.abs(delta * 100).toFixed(1)}%
            </div>
          )}
        </div>
      </div>

      <div className="relative mt-4" onMouseLeave={() => setHover(null)}>
        <svg
          viewBox={`0 0 ${W} ${height}`}
          preserveAspectRatio="none"
          className="w-full overflow-visible"
          style={{ height }}
        >
          <defs>
            <linearGradient id={`${id}-fill`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={STROKE[tone]} stopOpacity={0.32} />
              <stop offset="100%" stopColor={STROKE[tone]} stopOpacity={0} />
            </linearGradient>
          </defs>
          {[0.25, 0.5, 0.75].map((f) => (
            <line
              key={f}
              x1={0}
              x2={W}
              y1={height * f}
              y2={height * f}
              stroke="var(--line)"
              strokeDasharray="2 4"
              vectorEffect="non-scaling-stroke"
            />
          ))}
          {kind === "bar" ? (
            values.map((v, i) => {
              const bh = Math.max(2, (height - 6) * (Math.max(v, 0) / ceil));
              const bw = slot * 0.56;
              return (
                <motion.rect
                  key={i}
                  x={i * slot + (slot - bw) / 2}
                  width={bw}
                  rx={2}
                  initial={{ y: height, height: 0 }}
                  animate={{ y: height - bh, height: bh }}
                  transition={{
                    duration: 0.6,
                    delay: i * 0.025,
                    ease: [0.22, 1, 0.36, 1],
                  }}
                  fill={STROKE[tone]}
                  fillOpacity={i === idx ? 1 : 0.38}
                />
              );
            })
          ) : (
            <>
              <path
                d={`${line} L${W},${height} L0,${height} Z`}
                fill={`url(#${id}-fill)`}
              />
              <motion.path
                d={line}
                fill="none"
                stroke={STROKE[tone]}
                strokeWidth={2}
                vectorEffect="non-scaling-stroke"
                initial={{ pathLength: 0 }}
                animate={{ pathLength: 1 }}
                transition={{ duration: 1, ease: [0.22, 1, 0.36, 1] }}
              />
            </>
          )}
          {kind === "area" && pts[idx] && (
            <line
              x1={pts[idx][0]}
              x2={pts[idx][0]}
              y1={0}
              y2={height}
              stroke="var(--line-strong)"
              vectorEffect="non-scaling-stroke"
            />
          )}
          {values.map((_, i) => (
            <rect
              key={i}
              x={
                kind === "bar"
                  ? i * slot
                  : i * (W / Math.max(last, 1)) - W / Math.max(last, 1) / 2
              }
              y={0}
              width={kind === "bar" ? slot : W / Math.max(last, 1)}
              height={height}
              fill="transparent"
              onMouseEnter={() => setHover(i)}
            />
          ))}
        </svg>
        {kind === "area" && pts[idx] && (
          <span
            className="pointer-events-none absolute size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-bg"
            style={{
              left: `${(pts[idx][0] / W) * 100}%`,
              top: pts[idx][1],
              background: STROKE[tone],
              boxShadow: `0 0 10px ${STROKE[tone]}`,
            }}
          />
        )}
        <div className="mt-2 flex justify-between text-[10px] text-fg-4">
          <span>{monthLabel(months[0] ?? "")}</span>
          <span>{monthLabel(months[last] ?? "")}</span>
        </div>
      </div>
    </div>
  );
}
