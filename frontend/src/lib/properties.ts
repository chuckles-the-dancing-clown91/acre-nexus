// Small pure helpers for the Properties screens.

import type { Property } from "@/lib/types";

/** Case-insensitive match on name, address, city, state and manager. */
export function matchesProperty(p: Property, q: string): boolean {
  const needle = q.trim().toLowerCase();
  if (!needle) return true;
  return [p.name, p.address, p.city, p.state, p.manager]
    .filter(Boolean)
    .some((v) => v.toLowerCase().includes(needle));
}

/** Whole-percent occupancy, 0 for a property with no units. */
export function occupancyPct(units: number, occupied: number): number {
  return units ? Math.round((occupied * 100) / units) : 0;
}
