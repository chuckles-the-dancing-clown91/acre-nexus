import { describe, expect, it } from "vitest";
import type { Property } from "@/lib/types";
import { paintOrder } from "./geometry";
import { diagonalLots, occupancyTone, skylineBlocks } from "./skyline";

function property(id: string, units: number, occupied: number): Property {
  return {
    id,
    name: id,
    address: "",
    city: "",
    state: "",
    postal_code: "",
    photo_status: "none",
    photo_error: null,
    llc_id: null,
    units,
    occupied_units: occupied,
    occupancy: `${occupied}/${units}`,
    monthly_rent_cents: 0,
    monthly_rent_label: "$0",
    status: "stabilized",
    year_built: 2000,
    manager: "",
    property_type: "",
    strategy: "",
    workflow_stage: "",
    purchase_price_cents: null,
    acquired_on: null,
    image_url: null,
  };
}

describe("diagonalLots", () => {
  it("fills back-to-front diagonals", () => {
    expect(diagonalLots(5)).toEqual([
      [0, 0],
      [1, 0],
      [0, 1],
      [2, 0],
      [1, 1],
    ]);
  });

  it("returns distinct cells inside a square plot", () => {
    const lots = diagonalLots(40);
    const side = Math.ceil(Math.sqrt(40));
    expect(new Set(lots.map((l) => l.join(","))).size).toBe(40);
    for (const [x, y] of lots) {
      expect(x).toBeLessThan(side);
      expect(y).toBeLessThan(side);
    }
  });
});

describe("occupancyTone", () => {
  it("bands occupancy into healthy, watch, and problem", () => {
    expect(occupancyTone(24, 24)).toBe("accent");
    expect(occupancyTone(23, 24)).toBe("accent");
    expect(occupancyTone(9, 10)).toBe("warn");
    expect(occupancyTone(7, 10)).toBe("bad");
    expect(occupancyTone(0, 0)).toBe("accent");
  });
});

describe("skylineBlocks", () => {
  const props = [
    property("small", 6, 6),
    property("big", 24, 23),
    property("mid", 12, 11),
  ];
  const blocks = skylineBlocks(props);

  it("puts the largest property at the back and tallest", () => {
    expect(blocks[0].id).toBe("big");
    expect(blocks.map((b) => b.h)).toEqual(
      [...blocks.map((b) => b.h)].sort((a, b) => b - a)
    );
    expect(paintOrder(blocks)[0].id).toBe("big");
  });

  it("lights windows in proportion to occupancy", () => {
    expect(blocks.find((b) => b.id === "big")!.lit).toBeCloseTo(23 / 24);
    expect(blocks.find((b) => b.id === "small")!.lit).toBe(1);
  });

  it("never overlaps footprints", () => {
    for (const a of blocks) {
      for (const b of blocks) {
        if (a === b) continue;
        const apart =
          a.x + a.w <= b.x ||
          b.x + b.w <= a.x ||
          a.y + a.d <= b.y ||
          b.y + b.d <= a.y;
        expect(apart).toBe(true);
      }
    }
  });
});
