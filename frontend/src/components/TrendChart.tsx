// A small, dependency-free SVG trend chart (line or bars) for the financial
// dashboards. Theme-aware by construction: strokes/fills ride CSS variables
// so white-label accents and dark mode re-colour it like everything else.

import { Card } from "@/components/ui";
import { linePath, monthLabel, plotGeometry } from "@/lib/chart";

const W = 560;
const H = 170;
const PAD_X = 8;
const PAD_Y = 14;

export function TrendChart({
  title,
  months,
  values,
  kind = "line",
  format,
  color = "var(--accent)",
}: {
  title: string;
  /** `YYYY-MM`, oldest first (aligned with `values`). */
  months: string[];
  values: number[];
  kind?: "line" | "bar";
  /** Value formatter for the headline + axis ceiling. */
  format: (v: number) => string;
  /** Any CSS colour; defaults to the workspace accent. */
  color?: string;
}) {
  const geo = plotGeometry(values, W, H, PAD_X, PAD_Y);
  const latest = values[values.length - 1] ?? 0;
  const baseline = geo.y(0);
  // Label roughly every third month, always including the last.
  const labelled = months
    .map((m, i) => ({ m, i }))
    .filter(({ i }) => i === months.length - 1 || i % 3 === 0);

  return (
    <Card className="p-4">
      <div className="mb-1 flex items-baseline justify-between gap-3">
        <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
          {title}
        </span>
        <span className="font-display text-lg font-extrabold tracking-tight">
          {format(latest)}
        </span>
      </div>
      <svg
        viewBox={`0 0 ${W} ${H + 18}`}
        className="w-full"
        role="img"
        aria-label={`${title}, latest ${format(latest)}`}
      >
        {/* gridlines at 0 / 50 / 100% of the axis ceiling */}
        {[0, 0.5, 1].map((f) => (
          <line
            key={f}
            x1={PAD_X}
            x2={W - PAD_X}
            y1={geo.y(geo.ceiling * f)}
            y2={geo.y(geo.ceiling * f)}
            stroke="var(--line)"
            strokeWidth={1}
            strokeDasharray={f === 0 ? undefined : "3 4"}
          />
        ))}

        {kind === "bar" ? (
          values.map((v, i) => {
            const bw = Math.max(((W - PAD_X * 2) / values.length) * 0.55, 4);
            return (
              <rect
                key={i}
                x={geo.x(i) - bw / 2}
                y={geo.y(v)}
                width={bw}
                height={Math.max(baseline - geo.y(v), v > 0 ? 2 : 0)}
                rx={2}
                fill={color}
                opacity={i === values.length - 1 ? 1 : 0.55}
              />
            );
          })
        ) : (
          <>
            <path
              d={`${linePath(values, geo)} L${geo.x(values.length - 1).toFixed(1)},${baseline} L${geo.x(0).toFixed(1)},${baseline} Z`}
              fill={color}
              opacity={0.12}
            />
            <path
              d={linePath(values, geo)}
              fill="none"
              stroke={color}
              strokeWidth={2.5}
              strokeLinejoin="round"
              strokeLinecap="round"
            />
            <circle
              cx={geo.x(values.length - 1)}
              cy={geo.y(latest)}
              r={4}
              fill={color}
            />
          </>
        )}

        {labelled.map(({ m, i }) => (
          <text
            key={m}
            x={geo.x(i)}
            y={H + 13}
            textAnchor={
              i === 0 ? "start" : i === months.length - 1 ? "end" : "middle"
            }
            className="fill-ink-3"
            fontSize={11}
          >
            {monthLabel(m)}
          </text>
        ))}
      </svg>
    </Card>
  );
}
