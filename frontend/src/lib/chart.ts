// Pure helpers behind <TrendChart /> — kept out of the component so the
// scaling / formatting logic is unit-testable without a DOM.

/** Round `max` up to a "nice" axis ceiling (1 / 2 / 2.5 / 5 × 10^k). */
export function niceCeil(max: number): number {
  if (max <= 0) return 1;
  const exp = Math.floor(Math.log10(max));
  const base = Math.pow(10, exp);
  const unit = max / base;
  const nice =
    unit <= 1 ? 1 : unit <= 2 ? 2 : unit <= 2.5 ? 2.5 : unit <= 5 ? 5 : 10;
  return nice * base;
}

/** `"2026-07"` → `"Jul"`; January carries the year (`"Jan 26"`). */
export function monthLabel(month: string): string {
  const names = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ];
  const [y, m] = month.split("-");
  const idx = Number(m) - 1;
  if (!y || Number.isNaN(idx) || idx < 0 || idx > 11) return month;
  return idx === 0 ? `${names[idx]} ${y.slice(2)}` : names[idx];
}

/** Compact USD from cents: `$950`, `$1.8k`, `$2.26m`. */
export function compactUsd(cents: number): string {
  const sign = cents < 0 ? "-" : "";
  const dollars = Math.abs(cents) / 100;
  if (dollars >= 1_000_000)
    return `${sign}$${trimZero((dollars / 1_000_000).toFixed(2))}m`;
  if (dollars >= 1_000)
    return `${sign}$${trimZero((dollars / 1_000).toFixed(1))}k`;
  return `${sign}$${Math.round(dollars)}`;
}

/** Basis points → percentage label: `9500` → `"95%"`, `250` → `"2.5%"`. */
export function bpsLabel(bps: number): string {
  return `${trimZero((bps / 100).toFixed(1))}%`;
}

function trimZero(s: string): string {
  return s.replace(/\.?0+$/, "");
}

/** Chart geometry: map series values into an SVG plot area. */
export interface PlotGeometry {
  x: (index: number) => number;
  y: (value: number) => number;
  ceiling: number;
}

export function plotGeometry(
  values: number[],
  width: number,
  height: number,
  padX: number,
  padY: number
): PlotGeometry {
  const ceiling = niceCeil(Math.max(...values, 0));
  const innerW = width - padX * 2;
  const innerH = height - padY * 2;
  const step = values.length > 1 ? innerW / (values.length - 1) : 0;
  return {
    x: (i) => padX + i * step,
    y: (v) => padY + innerH - (Math.max(v, 0) / ceiling) * innerH,
    ceiling,
  };
}

/** An SVG path (`M … L …`) through every point of the series. */
export function linePath(values: number[], geo: PlotGeometry): string {
  return values
    .map(
      (v, i) =>
        `${i === 0 ? "M" : "L"}${geo.x(i).toFixed(1)},${geo.y(v).toFixed(1)}`
    )
    .join(" ");
}
