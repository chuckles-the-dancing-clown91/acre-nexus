// Lays a real portfolio out as an isometric skyline: one building per
// property, tallest at the back, height from unit count, lit windows from
// occupancy, and colour from occupancy health.

import type { Property } from "@/lib/types";
import type { IsoBlock, IsoTone } from "./geometry";

const LOT = 3.9;

export function occupancyTone(occupied: number, units: number): IsoTone {
  if (units <= 0) return "accent";
  const r = occupied / units;
  return r >= 0.95 ? "accent" : r >= 0.8 ? "warn" : "bad";
}

/** Grid cells in back-to-front diagonal order, filling a roughly square plot. */
export function diagonalLots(n: number): [number, number][] {
  const side = Math.max(1, Math.ceil(Math.sqrt(n)));
  const out: [number, number][] = [];
  for (let s = 0; out.length < n; s++) {
    for (let i = 0; i <= s && out.length < n; i++) {
      const x = s - i;
      const y = i;
      if (x < side && y < side) out.push([x, y]);
    }
  }
  return out;
}

export function skylineBlocks(properties: Property[]): IsoBlock[] {
  const sorted = [...properties].sort(
    (a, b) => b.units - a.units || a.name.localeCompare(b.name)
  );
  const maxUnits = Math.max(1, ...sorted.map((p) => p.units));
  const lots = diagonalLots(sorted.length);
  return sorted.map((p, i) => {
    const weight = Math.sqrt(Math.max(p.units, 1) / maxUnits);
    const size = 1.6 + 0.7 * weight;
    const h = 1.2 + 4.6 * weight;
    const [gx, gy] = lots[i];
    return {
      id: p.id,
      x: gx * LOT + (LOT - size) / 2,
      y: gy * LOT + (LOT - size) / 2,
      w: size,
      d: size,
      h,
      floors: Math.max(1, Math.round(h / 0.7)),
      lit: p.units > 0 ? p.occupied_units / p.units : 0,
      tone: occupancyTone(p.occupied_units, p.units),
    };
  });
}
