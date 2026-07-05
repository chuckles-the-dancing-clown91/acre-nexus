import { describe, expect, it } from "vitest";
import {
  bpsLabel,
  compactUsd,
  linePath,
  monthLabel,
  niceCeil,
  plotGeometry,
} from "./chart";

describe("niceCeil", () => {
  it("rounds up to nice axis ceilings", () => {
    expect(niceCeil(85)).toBe(100);
    expect(niceCeil(120)).toBe(200);
    expect(niceCeil(230)).toBe(250);
    expect(niceCeil(420)).toBe(500);
    expect(niceCeil(1000)).toBe(1000);
    expect(niceCeil(362000)).toBe(500000);
  });

  it("handles empty/zero series", () => {
    expect(niceCeil(0)).toBe(1);
    expect(niceCeil(-5)).toBe(1);
  });
});

describe("monthLabel", () => {
  it("shortens months and stamps the year on January", () => {
    expect(monthLabel("2026-07")).toBe("Jul");
    expect(monthLabel("2026-01")).toBe("Jan 26");
    expect(monthLabel("garbage")).toBe("garbage");
  });
});

describe("compactUsd", () => {
  it("compacts cents into readable dollars", () => {
    expect(compactUsd(95_000)).toBe("$950");
    expect(compactUsd(185_000)).toBe("$1.9k");
    expect(compactUsd(362_000)).toBe("$3.6k");
    expect(compactUsd(226_440_000)).toBe("$2.26m");
    expect(compactUsd(0)).toBe("$0");
    expect(compactUsd(-185_000)).toBe("-$1.9k");
  });
});

describe("bpsLabel", () => {
  it("formats basis points as percentages", () => {
    expect(bpsLabel(9500)).toBe("95%");
    expect(bpsLabel(250)).toBe("2.5%");
    expect(bpsLabel(0)).toBe("0%");
  });
});

describe("plotGeometry + linePath", () => {
  it("maps values into the padded plot area", () => {
    const geo = plotGeometry([0, 50, 100], 120, 60, 10, 10);
    expect(geo.ceiling).toBe(100);
    expect(geo.x(0)).toBe(10);
    expect(geo.x(2)).toBe(110);
    // Max value sits at the top pad, zero at the bottom.
    expect(geo.y(100)).toBe(10);
    expect(geo.y(0)).toBe(50);
  });

  it("builds a continuous path", () => {
    const geo = plotGeometry([1, 2], 100, 50, 0, 0);
    const d = linePath([1, 2], geo);
    expect(d.startsWith("M")).toBe(true);
    expect(d).toContain("L");
    expect(d.split("L").length).toBe(2);
  });
});
