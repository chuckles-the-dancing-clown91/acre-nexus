import { describe, expect, it } from "vitest";
import { matchesProperty, occupancyPct } from "./properties";
import type { Property } from "./types";

const p = {
  name: "Maple Court",
  address: "123 Maple Ct",
  city: "Portland",
  state: "OR",
  manager: "Sam Ortiz",
} as Property;

describe("property helpers", () => {
  it("matches on any visible field, ignoring case and spaces", () => {
    expect(matchesProperty(p, "")).toBe(true);
    expect(matchesProperty(p, "  maple ")).toBe(true);
    expect(matchesProperty(p, "portland")).toBe(true);
    expect(matchesProperty(p, "ortiz")).toBe(true);
    expect(matchesProperty(p, "seattle")).toBe(false);
  });
  it("rounds occupancy and copes with no units", () => {
    expect(occupancyPct(0, 0)).toBe(0);
    expect(occupancyPct(3, 2)).toBe(67);
    expect(occupancyPct(10, 10)).toBe(100);
  });
});
